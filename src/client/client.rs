//! STUN Client implementation.
//!
//! Sends STUN Binding Requests to a server and parses the response
//! to extract the client's reflexive (mapped) address. Supports unauthenticated,
//! short-term, and long-term credential modes.

use rand::RngCore;
use std::fmt;
use std::net::SocketAddr;
use std::time::{Duration, Instant};
use stun_protocol::integrity::{
    hmac_input, hmac_sha1, long_term_key as derive_long_term_key, short_term_key as shared_key,
};
use stun_protocol::{
    describe_message, is_comprehension_required, ErrorCode, MappedAddress, StunAttribute,
    StunAttributeType, StunMessage, StunMessageType, UnknownAttributes,
};
use tokio::net::UdpSocket;

/// SOFTWARE value advertised by this client in every request.
pub const SOFTWARE: &str = concat!("stunclient ", env!("CARGO_PKG_VERSION"));

/// Maximum number of signed retries after an authentication challenge.
pub const MAX_AUTH_RETRIES: u8 = 2;

/// Credential mode selected by the operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthMode {
    Disabled,
    ShortTerm,
    LongTerm,
}

impl AuthMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::ShortTerm => "short-term",
            Self::LongTerm => "long-term",
        }
    }
}

/// Client credentials. The password is never rendered by `Debug`.
#[derive(Clone)]
pub struct ClientAuth {
    mode: AuthMode,
    username: Option<String>,
    password: Option<String>,
}

impl fmt::Debug for ClientAuth {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ClientAuth")
            .field("mode", &self.mode.as_str())
            .field("username", &self.username)
            .field("password", &self.password.as_ref().map(|_| "<redacted>"))
            .finish()
    }
}

impl ClientAuth {
    pub fn disabled() -> Self {
        Self {
            mode: AuthMode::Disabled,
            username: None,
            password: None,
        }
    }

    pub fn short_term(username: impl Into<String>, password: impl Into<String>) -> Self {
        Self {
            mode: AuthMode::ShortTerm,
            username: Some(username.into()),
            password: Some(password.into()),
        }
    }

    pub fn long_term(username: impl Into<String>, password: impl Into<String>) -> Self {
        Self {
            mode: AuthMode::LongTerm,
            username: Some(username.into()),
            password: Some(password.into()),
        }
    }

    pub fn mode(&self) -> AuthMode {
        self.mode
    }

    pub fn username(&self) -> Option<&str> {
        self.username.as_deref()
    }

    pub fn password(&self) -> Option<&str> {
        self.password.as_deref()
    }

    fn short_term_key(&self) -> Result<Vec<u8>, ClientError> {
        self.password.as_deref().map(shared_key).ok_or_else(|| {
            ClientError::Protocol("short-term credentials require a password".into())
        })
    }

    fn long_term_key(&self, realm: &str) -> Result<[u8; 16], ClientError> {
        let username = self.username.as_deref().ok_or_else(|| {
            ClientError::Protocol("long-term credentials require a username".into())
        })?;
        let password = self.password.as_deref().ok_or_else(|| {
            ClientError::Protocol("long-term credentials require a password".into())
        })?;
        derive_long_term_key(username, realm, password)
            .map_err(|e| ClientError::Protocol(format!("failed to derive the long-term key: {e}")))
    }
}

/// REALM and NONCE pair carried by a 401 or 438 challenge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Challenge {
    pub realm: String,
    pub nonce: String,
}

/// Result of a successful STUN binding request.
#[derive(Debug, Clone)]
pub struct BindingResponse {
    /// The mapped (reflexive) address as seen by the STUN server.
    pub mapped_address: SocketAddr,
    /// Round-trip latency of the request.
    pub latency: Duration,
}

/// Errors that can occur during a STUN binding request.
#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    #[error("Connection timeout after {0} seconds")]
    Timeout(u64),

    #[error("Connection error: {0}")]
    Connection(String),

    #[error("Protocol error: {0}")]
    Protocol(String),

    #[error("Invalid server address: {0}")]
    InvalidAddress(String),

    #[error("{}", format_server_error(class, number, reason, unknown_attributes))]
    ServerError {
        class: u8,
        number: u8,
        reason: String,
        unknown_attributes: Vec<u16>,
    },
}

fn format_server_error(class: &u8, number: &u8, reason: &str, unknown: &[u16]) -> String {
    let status = u16::from(*class) * 100 + u16::from(*number);
    let list = if unknown.is_empty() {
        "none".to_string()
    } else {
        unknown
            .iter()
            .map(|t| format!("0x{:04X}", t))
            .collect::<Vec<_>>()
            .join(", ")
    };
    format!("server returned error {status} ({reason}); unknown attributes: {list}")
}

/// Generate a fresh 96-bit transaction ID from the operating system RNG.
pub fn generate_transaction_id() -> [u8; 12] {
    let mut id = [0u8; 12];
    rand::rngs::OsRng.fill_bytes(&mut id);
    id
}

/// Build a Binding Request for the given credentials and challenge state.
///
/// Every request carries SOFTWARE. Short-term requests also carry USERNAME and
/// MESSAGE-INTEGRITY; long-term requests stay unauthenticated until a challenge
/// supplies REALM and NONCE.
pub fn build_request(
    transaction_id: [u8; 12],
    auth: &ClientAuth,
    challenge: Option<&Challenge>,
) -> Result<(StunMessage, Vec<u8>), ClientError> {
    let mut request = StunMessage::new(StunMessageType::Request, transaction_id);
    let mut key: Option<Vec<u8>> = None;

    match auth.mode {
        AuthMode::Disabled => {}
        AuthMode::ShortTerm => {
            let username = auth.username.as_deref().ok_or_else(|| {
                ClientError::Protocol("short-term credentials require a username".into())
            })?;
            request.add_attribute(StunAttribute::new(
                StunAttributeType::Username,
                username.as_bytes().to_vec(),
            ));
            key = Some(auth.short_term_key()?);
        }
        AuthMode::LongTerm => {
            if let Some(challenge) = challenge {
                let username = auth.username.as_deref().ok_or_else(|| {
                    ClientError::Protocol("long-term credentials require a username".into())
                })?;
                request.add_attribute(StunAttribute::new(
                    StunAttributeType::Username,
                    username.as_bytes().to_vec(),
                ));
                request.add_attribute(StunAttribute::new(
                    StunAttributeType::Realm,
                    challenge.realm.as_bytes().to_vec(),
                ));
                request.add_attribute(StunAttribute::new(
                    StunAttributeType::Nonce,
                    challenge.nonce.as_bytes().to_vec(),
                ));
                key = Some(auth.long_term_key(&challenge.realm)?.to_vec());
            }
        }
    }

    request.add_attribute(StunAttribute::new(
        StunAttributeType::Software,
        SOFTWARE.as_bytes().to_vec(),
    ));

    let bytes = match &key {
        Some(key) => {
            request.add_attribute(StunAttribute::new(
                StunAttributeType::MessageIntegrity,
                vec![0u8; 20],
            ));
            let mi_offset = request
                .attribute_offset(StunAttributeType::MessageIntegrity as u16)
                .expect("MESSAGE-INTEGRITY was just added");
            let mut buffer = request.encode();
            let digest = hmac_sha1(key, &hmac_input(&buffer, mi_offset));
            buffer[mi_offset + 4..mi_offset + 24].copy_from_slice(&digest);
            buffer
        }
        None => request.encode(),
    };
    Ok((request, bytes))
}

/// Send a STUN Binding Request without authentication.
pub async fn send_binding_request(
    server_addr: SocketAddr,
    timeout: Duration,
) -> Result<BindingResponse, ClientError> {
    send_binding_request_with_auth(server_addr, timeout, &ClientAuth::disabled()).await
}

/// Send a STUN Binding Request with the configured credential mode.
pub async fn send_binding_request_with_auth(
    server_addr: SocketAddr,
    timeout: Duration,
    auth: &ClientAuth,
) -> Result<BindingResponse, ClientError> {
    log::debug!(
        "auth={} user={}",
        auth.mode.as_str(),
        auth.username.as_deref().unwrap_or("-")
    );

    if auth.mode == AuthMode::LongTerm {
        return long_term_flow(server_addr, timeout, auth).await;
    }

    let transaction_id = generate_transaction_id();
    let (message, bytes) = build_request(transaction_id, auth, None)?;
    log_request(&message, &bytes);

    let key = match auth.mode {
        AuthMode::ShortTerm => Some(auth.short_term_key()?),
        _ => None,
    };
    send_single(server_addr, timeout, transaction_id, &bytes, key.as_deref()).await
}

/// Send a STUN Binding Request with fallback to multiple addresses.
///
/// Tries each address in order until one succeeds. Returns the first successful
/// response or an error if all addresses fail.
pub async fn send_binding_request_with_fallback(
    server_addrs: &[SocketAddr],
    timeout: Duration,
) -> Result<BindingResponse, ClientError> {
    send_binding_request_with_fallback_and_auth(server_addrs, timeout, &ClientAuth::disabled())
        .await
}

/// Send a STUN Binding Request with fallback and credentials.
pub async fn send_binding_request_with_fallback_and_auth(
    server_addrs: &[SocketAddr],
    timeout: Duration,
    auth: &ClientAuth,
) -> Result<BindingResponse, ClientError> {
    if server_addrs.is_empty() {
        return Err(ClientError::InvalidAddress(
            "no addresses provided".to_string(),
        ));
    }

    let mut last_error = None;

    for addr in server_addrs {
        match send_binding_request_with_auth(*addr, timeout, auth).await {
            Ok(response) => return Ok(response),
            Err(e) => {
                log::debug!("Failed to connect to {}: {}", addr, e);
                last_error = Some(e);
            }
        }
    }

    Err(last_error.unwrap_or_else(|| ClientError::Connection("all addresses failed".to_string())))
}

/// Run the long-term credential flow: one unauthenticated request, then at
/// most `MAX_AUTH_RETRIES` signed retries that adopt fresh NONCE values.
async fn long_term_flow(
    server_addr: SocketAddr,
    timeout: Duration,
    auth: &ClientAuth,
) -> Result<BindingResponse, ClientError> {
    let transaction_id = generate_transaction_id();
    let (message, bytes) = build_request(transaction_id, auth, None)?;
    log_request(&message, &bytes);

    let (data, latency) = exchange(server_addr, timeout, &bytes).await?;
    let (mut result, mut challenge) =
        parse_response_detailed(&data, &transaction_id, latency, None);
    let mut retries = 0u8;

    loop {
        match result {
            Ok(response) => return Ok(response),
            Err(ClientError::ServerError {
                class,
                number,
                reason,
                unknown_attributes,
            }) if is_challenge(class, number) => {
                let Some(current) = challenge.take() else {
                    return Err(ClientError::Protocol(
                        "authentication challenge missing REALM or NONCE".to_string(),
                    ));
                };
                if retries >= MAX_AUTH_RETRIES {
                    return Err(ClientError::ServerError {
                        class,
                        number,
                        reason,
                        unknown_attributes,
                    });
                }
                retries += 1;

                let next_id = generate_transaction_id();
                let (message, bytes) = build_request(next_id, auth, Some(&current))?;
                log_request(&message, &bytes);
                let key = auth.long_term_key(&current.realm)?;

                let (data, latency) = exchange(server_addr, timeout, &bytes).await?;
                let parsed = parse_response_detailed(&data, &next_id, latency, Some(&key));
                result = parsed.0;
                challenge = parsed.1;
            }
            Err(err) => return Err(err),
        }
    }
}

/// Send one request and parse the matching response.
async fn send_single(
    server_addr: SocketAddr,
    timeout: Duration,
    transaction_id: [u8; 12],
    request_bytes: &[u8],
    expected_key: Option<&[u8]>,
) -> Result<BindingResponse, ClientError> {
    let (data, latency) = exchange(server_addr, timeout, request_bytes).await?;
    parse_response_detailed(&data, &transaction_id, latency, expected_key).0
}

/// Bind a socket, send a request, and wait for a response from the server.
async fn exchange(
    server_addr: SocketAddr,
    timeout: Duration,
    request_bytes: &[u8],
) -> Result<(Vec<u8>, Duration), ClientError> {
    let local_addr: SocketAddr = if server_addr.is_ipv4() {
        "0.0.0.0:0".parse().unwrap()
    } else {
        "[::]:0".parse().unwrap()
    };

    let socket = UdpSocket::bind(local_addr)
        .await
        .map_err(|e| ClientError::Connection(format!("failed to bind socket: {}", e)))?;

    let start = Instant::now();
    socket
        .send_to(request_bytes, server_addr)
        .await
        .map_err(|e| ClientError::Connection(format!("failed to send request: {}", e)))?;

    let mut recv_buf = vec![0u8; 65536];
    let recv_result = tokio::time::timeout(timeout, socket.recv_from(&mut recv_buf)).await;
    let latency = start.elapsed();

    match recv_result {
        Ok(Ok((len, from_addr))) => {
            log::debug!("Received {} bytes from {}", len, from_addr);
            log::trace!("Received: {:02x?}", &recv_buf[..len]);
            if from_addr != server_addr {
                return Err(ClientError::Protocol(format!(
                    "response from unexpected address: {} (expected {})",
                    from_addr, server_addr
                )));
            }
            Ok((recv_buf[..len].to_vec(), latency))
        }
        Ok(Err(e)) => Err(ClientError::Connection(format!("recv error: {}", e))),
        Err(_) => Err(ClientError::Timeout(timeout.as_secs())),
    }
}

fn log_request(message: &StunMessage, bytes: &[u8]) {
    log::debug!("Sending: {}", describe_message(message));
    log::trace!("Sending: {:02x?}", bytes);
}

/// Parse a STUN Binding Response and extract the mapped address.
#[cfg(test)]
fn parse_response(
    data: &[u8],
    expected_transaction_id: &[u8; 12],
    latency: Duration,
) -> Result<BindingResponse, ClientError> {
    parse_response_detailed(data, expected_transaction_id, latency, None).0
}

/// Parse a response, returning any authentication challenge it carries.
fn parse_response_detailed(
    data: &[u8],
    expected_transaction_id: &[u8; 12],
    latency: Duration,
    expected_key: Option<&[u8]>,
) -> (Result<BindingResponse, ClientError>, Option<Challenge>) {
    let response = match StunMessage::decode(data) {
        Ok(response) => response,
        Err(e) => {
            log::debug!("Failed to decode STUN message ({} bytes)", data.len());
            return (
                Err(ClientError::Protocol(format!(
                    "failed to decode response: {}",
                    e
                ))),
                None,
            );
        }
    };

    log::debug!("Received: {}", describe_message(&response));

    if response.transaction_id != *expected_transaction_id {
        return (
            Err(ClientError::Protocol("transaction ID mismatch".to_string())),
            None,
        );
    }

    // A FINGERPRINT, when present, must be the final attribute, exactly 4
    // bytes, and CRC-correct (RFC 5389 section 15.5).
    if response.attribute_count(StunAttributeType::Fingerprint as u16) > 0
        && !StunMessage::verify_message_fingerprint(data)
    {
        return (
            Err(ClientError::Protocol(
                "invalid FINGERPRINT in response".to_string(),
            )),
            None,
        );
    }

    match response.message_type {
        StunMessageType::ErrorIndication => parse_error_response(data, &response, expected_key),
        StunMessageType::Response => {
            let result = parse_success_response(data, &response, latency, expected_key);
            (result, None)
        }
        StunMessageType::Request => (
            Err(ClientError::Protocol(format!(
                "unexpected message type: {:?}",
                response.message_type
            ))),
            None,
        ),
    }
}

fn parse_error_response(
    data: &[u8],
    response: &StunMessage,
    expected_key: Option<&[u8]>,
) -> (Result<BindingResponse, ClientError>, Option<Challenge>) {
    let code = response
        .attributes
        .iter()
        .find(|a| a.attribute_type == StunAttributeType::ErrorCode)
        .and_then(|a| ErrorCode::decode_value(&a.attribute_value).ok());

    let Some(code) = code else {
        return (
            Err(ClientError::Protocol(
                "error response missing valid ERROR-CODE".to_string(),
            )),
            None,
        );
    };

    if !(3..=6).contains(&code.class) || code.number > 99 {
        return (
            Err(ClientError::Protocol(format!(
                "error response with invalid ERROR-CODE {}{}",
                code.class, code.number
            ))),
            None,
        );
    }

    let challenge = if is_challenge(code.class, code.number) {
        let realm = text_attribute(response, StunAttributeType::Realm);
        let nonce = text_attribute(response, StunAttributeType::Nonce);
        match (realm, nonce) {
            (Some(realm), Some(nonce)) => Some(Challenge {
                realm: realm.to_string(),
                nonce: nonce.to_string(),
            }),
            _ => None,
        }
    } else {
        None
    };

    // RFC 5389 sections 10.1.2 and 10.2.2: failure responses are unsigned, so
    // only non-challenge errors are validated against the request key.
    let is_challenge = is_challenge(code.class, code.number);
    if let Some(key) = expected_key {
        if !is_challenge && !StunMessage::verify_message_integrity(data, key) {
            return (
                Err(ClientError::Protocol(
                    "response MESSAGE-INTEGRITY missing or invalid".to_string(),
                )),
                challenge,
            );
        }
    }

    let unknown_attributes = response
        .attributes
        .iter()
        .find(|a| a.attribute_type == StunAttributeType::UnknownAttributes)
        .and_then(|a| UnknownAttributes::decode_value(&a.attribute_value).ok())
        .map(|list| list.types)
        .unwrap_or_default();

    (
        Err(ClientError::ServerError {
            class: code.class,
            number: code.number,
            reason: code.reason,
            unknown_attributes,
        }),
        challenge,
    )
}

fn parse_success_response(
    data: &[u8],
    response: &StunMessage,
    latency: Duration,
    expected_key: Option<&[u8]>,
) -> Result<BindingResponse, ClientError> {
    if let Some(key) = expected_key {
        if !StunMessage::verify_message_integrity(data, key) {
            return Err(ClientError::Protocol(
                "response MESSAGE-INTEGRITY missing or invalid".to_string(),
            ));
        }
    }

    // A success response carrying a comprehension-required attribute the
    // client does not recognize cannot be trusted (RFC 5389 section 7.3.1).
    if let Some(attr) = response
        .attributes
        .iter()
        .find(|a| is_comprehension_required(a.raw_type) && !client_recognizes(a.raw_type))
    {
        return Err(ClientError::Protocol(format!(
            "success response contains unknown required attribute 0x{:04X}",
            attr.raw_type
        )));
    }

    let xor_attr = response
        .attributes
        .iter()
        .find(|a| a.attribute_type == StunAttributeType::XorMappedAddress)
        .ok_or_else(|| ClientError::Protocol("response missing XOR-MAPPED-ADDRESS".to_string()))?;

    let mapped = MappedAddress::decode_xor(
        &xor_attr.attribute_value,
        response.magic_cookie,
        &response.transaction_id,
    )
    .map_err(|e| ClientError::Protocol(format!("failed to decode mapped address: {}", e)))?;

    Ok(BindingResponse {
        mapped_address: SocketAddr::new(mapped.address, mapped.port),
        latency,
    })
}

fn is_challenge(class: u8, number: u8) -> bool {
    matches!((class, number), (4, 1) | (4, 38))
}

fn text_attribute(message: &StunMessage, attribute_type: StunAttributeType) -> Option<&str> {
    message
        .attributes
        .iter()
        .find(|attr| attr.attribute_type == attribute_type)
        .and_then(|attr| std::str::from_utf8(&attr.attribute_value).ok())
}

/// Comprehension-required attributes this client understands in a response.
fn client_recognizes(raw_type: u16) -> bool {
    matches!(
        raw_type,
        0x0001 | 0x0006 | 0x0008 | 0x0009 | 0x000A | 0x0014 | 0x0015 | 0x0020
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_transaction_id() {
        let id1 = generate_transaction_id();
        let id2 = generate_transaction_id();
        assert_ne!(id1, id2);
        assert_eq!(id1.len(), 12);
    }

    #[test]
    fn test_build_request_disabled_carries_software_only() {
        let (message, bytes) = build_request([0x42; 12], &ClientAuth::disabled(), None).unwrap();

        assert_eq!(
            message.attribute_count(StunAttributeType::Software as u16),
            1
        );
        assert_eq!(
            message.attribute_count(StunAttributeType::MessageIntegrity as u16),
            0
        );
        assert_eq!(
            message.attribute_count(StunAttributeType::Username as u16),
            0
        );
        assert_eq!(
            StunMessage::decode(&bytes).unwrap().transaction_id,
            [0x42; 12]
        );
    }

    #[test]
    fn test_build_request_short_term_is_signed() {
        let auth = ClientAuth::short_term("alice", "s3cret");
        let (message, bytes) = build_request([0x42; 12], &auth, None).unwrap();

        assert_eq!(
            text_attribute(&message, StunAttributeType::Username),
            Some("alice")
        );
        assert_eq!(
            message.attribute_count(StunAttributeType::MessageIntegrity as u16),
            1
        );
        assert!(StunMessage::verify_message_integrity(&bytes, b"s3cret"));
    }

    #[test]
    fn test_build_request_long_term_unsigned_until_challenged() {
        let auth = ClientAuth::long_term("alice", "s3cret");
        let (message, bytes) = build_request([0x42; 12], &auth, None).unwrap();

        assert_eq!(
            message.attribute_count(StunAttributeType::MessageIntegrity as u16),
            0
        );
        assert_eq!(message.attribute_count(StunAttributeType::Nonce as u16), 0);
        assert!(!StunMessage::verify_message_integrity(&bytes, b"s3cret"));
    }

    #[test]
    fn test_build_request_long_term_signed_when_challenged() {
        let auth = ClientAuth::long_term("alice", "s3cret");
        let challenge = Challenge {
            realm: "example.org".to_string(),
            nonce: "nonce-1".to_string(),
        };
        let (message, bytes) = build_request([0x42; 12], &auth, Some(&challenge)).unwrap();

        assert_eq!(
            text_attribute(&message, StunAttributeType::Username),
            Some("alice")
        );
        assert_eq!(
            text_attribute(&message, StunAttributeType::Realm),
            Some("example.org")
        );
        assert_eq!(
            text_attribute(&message, StunAttributeType::Nonce),
            Some("nonce-1")
        );
        let key = derive_long_term_key("alice", "example.org", "s3cret").unwrap();
        assert!(StunMessage::verify_message_integrity(&bytes, &key));
    }

    #[test]
    fn test_auth_debug_redacts_password() {
        let auth = ClientAuth::short_term("alice", "s3cret-password");
        let rendered = format!("{:?}", auth);
        assert!(rendered.contains("short-term"));
        assert!(rendered.contains("alice"));
        assert!(!rendered.contains("s3cret-password"));
        assert!(rendered.contains("redacted"));
        assert_eq!(auth.password(), Some("s3cret-password"));
    }

    #[test]
    fn test_parse_response_valid() {
        // Build a valid response.
        let transaction_id = [0x42; 12];
        let mut response = StunMessage::new(StunMessageType::Response, transaction_id);

        // Add XOR-MAPPED-ADDRESS for 192.168.1.100:5000.
        let mapped = MappedAddress::new("192.168.1.100".parse().unwrap(), 5000);
        let xor_value = mapped.encode_xor(response.magic_cookie, &transaction_id);
        let attr =
            stun_protocol::StunAttribute::new(StunAttributeType::XorMappedAddress, xor_value);
        response.add_attribute(attr);

        let response_bytes = response.encode();
        let latency = Duration::from_millis(5);

        let result = parse_response(&response_bytes, &transaction_id, latency).unwrap();
        assert_eq!(
            result.mapped_address,
            "192.168.1.100:5000".parse::<SocketAddr>().unwrap()
        );
        assert_eq!(result.latency, latency);
    }

    #[test]
    fn test_parse_response_wrong_transaction_id() {
        let transaction_id = [0x42; 12];
        let response = StunMessage::new(StunMessageType::Response, transaction_id);
        let response_bytes = response.encode();

        let wrong_id = [0x99; 12];
        let result = parse_response(&response_bytes, &wrong_id, Duration::from_millis(1));
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ClientError::Protocol(_)));
    }

    #[test]
    fn test_parse_response_wrong_message_type() {
        let transaction_id = [0x42; 12];
        let request = StunMessage::new(StunMessageType::Request, transaction_id);
        let request_bytes = request.encode();

        let result = parse_response(&request_bytes, &transaction_id, Duration::from_millis(1));
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ClientError::Protocol(_)));
    }

    #[test]
    fn test_parse_response_missing_attribute() {
        let transaction_id = [0x42; 12];
        let response = StunMessage::new(StunMessageType::Response, transaction_id);
        // No attributes added.
        let response_bytes = response.encode();

        let result = parse_response(&response_bytes, &transaction_id, Duration::from_millis(1));
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ClientError::Protocol(_)));
    }

    #[test]
    fn test_parse_response_invalid_data() {
        let transaction_id = [0x42; 12];
        let result = parse_response(&[0, 1, 2], &transaction_id, Duration::from_millis(1));
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ClientError::Protocol(_)));
    }

    fn raw_attribute(raw_type: u16, value: &[u8]) -> stun_protocol::StunAttribute {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&raw_type.to_be_bytes());
        bytes.extend_from_slice(&(value.len() as u16).to_be_bytes());
        bytes.extend_from_slice(value);
        stun_protocol::StunAttribute::decode(&bytes).unwrap()
    }

    fn error_response(transaction_id: [u8; 12], unknown: Option<Vec<u16>>) -> Vec<u8> {
        use stun_protocol::{ErrorCode, UnknownAttributes};

        let mut response = StunMessage::new(StunMessageType::ErrorIndication, transaction_id);
        let code = ErrorCode::new(4, 20, "Unknown Attribute".to_string());
        response.add_attribute(stun_protocol::StunAttribute::new(
            StunAttributeType::ErrorCode,
            code.encode_value(),
        ));
        if let Some(types) = unknown {
            response.add_attribute(stun_protocol::StunAttribute::new(
                StunAttributeType::UnknownAttributes,
                UnknownAttributes::new(types).encode_value(),
            ));
        }
        response.encode()
    }

    #[test]
    fn test_parse_response_420_with_unknown_attributes() {
        let transaction_id = [0x42; 12];
        let bytes = error_response(transaction_id, Some(vec![0x0024]));

        let err = parse_response(&bytes, &transaction_id, Duration::from_millis(1)).unwrap_err();

        match err {
            ClientError::ServerError {
                class,
                number,
                reason,
                unknown_attributes,
            } => {
                assert_eq!(class, 4);
                assert_eq!(number, 20);
                assert_eq!(reason, "Unknown Attribute");
                assert_eq!(unknown_attributes, vec![0x0024]);
            }
            other => panic!("unexpected error: {:?}", other),
        }
    }

    #[test]
    fn test_parse_response_420_without_unknown_attributes() {
        let transaction_id = [0x42; 12];
        let bytes = error_response(transaction_id, None);

        let err = parse_response(&bytes, &transaction_id, Duration::from_millis(1)).unwrap_err();

        match err {
            ClientError::ServerError {
                class,
                number,
                unknown_attributes,
                ..
            } => {
                assert_eq!(class, 4);
                assert_eq!(number, 20);
                assert!(unknown_attributes.is_empty());
            }
            other => panic!("unexpected error: {:?}", other),
        }
    }

    #[test]
    fn test_parse_response_error_without_error_code() {
        let transaction_id = [0x42; 12];
        let response = StunMessage::new(StunMessageType::ErrorIndication, transaction_id);

        let result = parse_response(
            &response.encode(),
            &transaction_id,
            Duration::from_millis(1),
        );
        assert!(matches!(result.unwrap_err(), ClientError::Protocol(_)));
    }

    #[test]
    fn test_parse_response_error_with_wrong_transaction_id() {
        let bytes = error_response([0x42; 12], Some(vec![0x0024]));
        let wrong_id = [0x99; 12];

        let result = parse_response(&bytes, &wrong_id, Duration::from_millis(1));
        assert!(matches!(result.unwrap_err(), ClientError::Protocol(_)));
    }

    fn status_error_response(transaction_id: [u8; 12], class: u8, number: u8) -> Vec<u8> {
        let mut response = StunMessage::new(StunMessageType::ErrorIndication, transaction_id);
        let code = ErrorCode::new(class, number, "bad".to_string());
        response.add_attribute(StunAttribute::new(
            StunAttributeType::ErrorCode,
            code.encode_value(),
        ));
        response.encode()
    }

    #[test]
    fn test_parse_response_error_class_out_of_range() {
        for class in [0, 2, 7] {
            let transaction_id = [0x42; 12];
            let bytes = status_error_response(transaction_id, class, 0);
            let result = parse_response(&bytes, &transaction_id, Duration::from_millis(1));
            assert!(
                matches!(result.unwrap_err(), ClientError::Protocol(_)),
                "class {class} must be rejected"
            );
        }
    }

    #[test]
    fn test_parse_response_error_number_above_99() {
        let transaction_id = [0x42; 12];
        let bytes = status_error_response(transaction_id, 4, 100);
        let result = parse_response(&bytes, &transaction_id, Duration::from_millis(1));
        assert!(matches!(result.unwrap_err(), ClientError::Protocol(_)));
    }

    #[test]
    fn test_parse_response_success_with_unknown_optional_attribute() {
        let transaction_id = [0x42; 12];
        let mut response = StunMessage::new(StunMessageType::Response, transaction_id);
        let mapped = MappedAddress::new("192.0.2.1".parse().unwrap(), 32853);
        let xor_value = mapped.encode_xor(response.magic_cookie, &transaction_id);
        response.add_attribute(stun_protocol::StunAttribute::new(
            StunAttributeType::XorMappedAddress,
            xor_value,
        ));
        response.add_attribute(raw_attribute(0x80FF, &[0xDE, 0xAD, 0xBE, 0xEF]));

        let result = parse_response(
            &response.encode(),
            &transaction_id,
            Duration::from_millis(1),
        )
        .unwrap();
        assert_eq!(
            result.mapped_address,
            "192.0.2.1:32853".parse::<SocketAddr>().unwrap()
        );
    }

    #[test]
    fn test_parse_response_success_with_unknown_required_attribute() {
        let transaction_id = [0x42; 12];
        let mut response = StunMessage::new(StunMessageType::Response, transaction_id);
        let mapped = MappedAddress::new("192.0.2.1".parse().unwrap(), 32853);
        let xor_value = mapped.encode_xor(response.magic_cookie, &transaction_id);
        response.add_attribute(stun_protocol::StunAttribute::new(
            StunAttributeType::XorMappedAddress,
            xor_value,
        ));
        response.add_attribute(raw_attribute(0x0024, &[0x00, 0x00, 0x00, 0x00]));

        let result = parse_response(
            &response.encode(),
            &transaction_id,
            Duration::from_millis(1),
        );
        assert!(matches!(result.unwrap_err(), ClientError::Protocol(_)));
    }

    #[test]
    fn test_signed_response_requires_valid_integrity() {
        let transaction_id = [0x42; 12];
        let mut response = StunMessage::new(StunMessageType::Response, transaction_id);
        let mapped = MappedAddress::new("192.0.2.1".parse().unwrap(), 32853);
        let xor_value = mapped.encode_xor(response.magic_cookie, &transaction_id);
        response.add_attribute(stun_protocol::StunAttribute::new(
            StunAttributeType::XorMappedAddress,
            xor_value,
        ));

        let unsigned = response.encode();
        let (result, _) = parse_response_detailed(
            &unsigned,
            &transaction_id,
            Duration::from_millis(1),
            Some(b"key"),
        );
        assert!(matches!(result.unwrap_err(), ClientError::Protocol(_)));

        let signed = response.encode_with_integrity(b"key");
        let (result, _) = parse_response_detailed(
            &signed,
            &transaction_id,
            Duration::from_millis(1),
            Some(b"key"),
        );
        assert!(result.is_ok());
    }
}
