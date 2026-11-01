//! Benchmark execution engine.
//!
//! Sends STUN Binding Requests at a target QPS rate, collects latency
//! measurements, and reports progress and final results.

use crate::stats::{BenchmarkStats, LatencyCollector};
use rand::RngCore;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use stun_protocol::integrity;
use stun_protocol::{
    describe_message, ErrorCode, StunAttribute, StunAttributeType, StunMessage, StunMessageType,
};
use tokio::net::UdpSocket;

/// How long an unanswered request is tracked before being counted as failed.
const PENDING_TTL: Duration = Duration::from_secs(5);

/// Timeout for each long-term challenge bootstrap exchange.
const CHALLENGE_TIMEOUT: Duration = Duration::from_secs(2);

/// Number of attempts to obtain a long-term challenge.
const CHALLENGE_ATTEMPTS: u32 = 3;

/// Maps an outstanding transaction ID to the instant its request was sent.
type PendingMap = Arc<Mutex<HashMap<[u8; 12], Instant>>>;

/// Upper bound for pre-allocating latency samples (each `Duration` is 16 bytes).
const MAX_PREALLOCATED_SAMPLES: usize = 8 * 1024 * 1024;

/// Maximum REALM length in characters (RFC 5389 section 15.7).
const MAX_REALM_CHARS: usize = 127;

/// Socket buffer sizes requested for the benchmark socket.
///
/// The kernel silently caps these at `net.core.rmem_max` / `net.core.wmem_max`.
const UDP_RECV_BUFFER_SIZE: usize = 4 * 1024 * 1024;
const UDP_SEND_BUFFER_SIZE: usize = 4 * 1024 * 1024;

/// Pre-allocate room for expected latency samples so a long run does not stall
/// the receiver with repeated vector growth.
fn expected_sample_capacity(config: &BenchConfig) -> usize {
    let expected = match config.request_count {
        Some(count) => count,
        None => config
            .duration
            .map(|duration| config.target_qps as f64 * duration.as_secs_f64())
            .unwrap_or(0.0) as u64,
    };
    usize::try_from(expected)
        .unwrap_or(usize::MAX)
        .min(MAX_PREALLOCATED_SAMPLES)
}

/// Bind the benchmark socket with explicit kernel buffer sizes.
fn bind_udp_socket(
    addr: SocketAddr,
    recv_size: usize,
    send_size: usize,
) -> std::io::Result<UdpSocket> {
    let domain = if addr.is_ipv4() {
        socket2::Domain::IPV4
    } else {
        socket2::Domain::IPV6
    };
    let socket = socket2::Socket::new(domain, socket2::Type::DGRAM, Some(socket2::Protocol::UDP))?;
    socket.set_nonblocking(true)?;
    socket.set_recv_buffer_size(recv_size)?;
    socket.set_send_buffer_size(send_size)?;
    socket.bind(&addr.into())?;
    UdpSocket::from_std(socket.into())
}

/// Configuration for a benchmark run.
#[derive(Debug, Clone)]
pub struct BenchConfig {
    /// Target server addresses (supports multiple for fallback).
    pub server_addrs: Vec<std::net::SocketAddr>,
    /// Target queries per second.
    pub target_qps: u32,
    /// Test duration (if specified).
    pub duration: Option<Duration>,
    /// Total request count (if specified, alternative to duration).
    pub request_count: Option<u64>,
    /// Append a terminal FINGERPRINT to every request.
    pub fingerprint: bool,
    /// Credentials used to sign requests and responses.
    pub auth: BenchAuth,
}

/// Credential mode for benchmark traffic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthMode {
    Disabled,
    ShortTerm,
    LongTerm,
}

impl AuthMode {
    pub fn as_str(self) -> &'static str {
        match self {
            AuthMode::Disabled => "disabled",
            AuthMode::ShortTerm => "short-term",
            AuthMode::LongTerm => "long-term",
        }
    }
}

/// Benchmark credentials as supplied by the command line.
#[derive(Debug, Clone)]
pub struct BenchAuth {
    mode: AuthMode,
    username: Option<String>,
    password: Option<String>,
    realm: Option<String>,
}

impl BenchAuth {
    pub fn disabled() -> Self {
        Self {
            mode: AuthMode::Disabled,
            username: None,
            password: None,
            realm: None,
        }
    }

    pub fn short_term(username: impl Into<String>, password: impl Into<String>) -> Self {
        Self {
            mode: AuthMode::ShortTerm,
            username: Some(username.into()),
            password: Some(password.into()),
            realm: None,
        }
    }

    pub fn long_term(username: impl Into<String>, password: impl Into<String>) -> Self {
        Self {
            mode: AuthMode::LongTerm,
            username: Some(username.into()),
            password: Some(password.into()),
            realm: None,
        }
    }

    pub fn long_term_with_realm(
        username: impl Into<String>,
        password: impl Into<String>,
        realm: impl Into<String>,
    ) -> Self {
        Self {
            mode: AuthMode::LongTerm,
            username: Some(username.into()),
            password: Some(password.into()),
            realm: Some(realm.into()),
        }
    }

    pub fn realm(&self) -> Option<&str> {
        self.realm.as_deref()
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

    pub fn prepare(&self) -> Result<PreparedAuth, String> {
        match self.mode {
            AuthMode::Disabled => Ok(PreparedAuth {
                mode: AuthMode::Disabled,
                username: None,
                password: None,
                realm: None,
            }),
            AuthMode::ShortTerm | AuthMode::LongTerm => {
                let username = self
                    .username
                    .as_deref()
                    .filter(|name| !name.is_empty())
                    .ok_or_else(|| {
                        "--username is required when authentication is enabled".to_string()
                    })?;
                let password = self
                    .password
                    .as_deref()
                    .filter(|secret| !secret.is_empty())
                    .ok_or_else(|| {
                        "--password or STUNBENCH_PASSWORD is required when authentication is enabled"
                            .to_string()
                    })?;
                // RFC 5389 sections 15.3 and 15.4: USERNAME and passwords are
                // SASLprep prepared before use.
                let username = integrity::saslprep(username)
                    .map_err(|e| format!("failed to prepare the username: {e}"))?;
                let password = integrity::saslprep(password)
                    .map_err(|e| format!("failed to prepare the password: {e}"))?;
                Ok(PreparedAuth {
                    mode: self.mode,
                    username: Some(username),
                    password: Some(password),
                    realm: self.realm.clone(),
                })
            }
        }
    }
}

/// Credentials validated once per run.
#[derive(Debug, Clone)]
pub struct PreparedAuth {
    mode: AuthMode,
    username: Option<String>,
    password: Option<String>,
    realm: Option<String>,
}

impl PreparedAuth {
    pub fn mode(&self) -> AuthMode {
        self.mode
    }

    /// REALM to authenticate with: the configured override when present,
    /// otherwise the server-proposed challenge realm. The result is SASLprep
    /// prepared and length-checked (RFC 5389 section 15.7).
    fn prepared_realm(&self, challenge_realm: &str) -> Result<String, String> {
        let realm = self.realm.as_deref().unwrap_or(challenge_realm);
        let prepared =
            integrity::saslprep(realm).map_err(|e| format!("failed to prepare the realm: {e}"))?;
        if prepared.chars().count() > MAX_REALM_CHARS {
            return Err("realm exceeds the 127-character REALM limit".to_string());
        }
        Ok(prepared)
    }
}

/// REALM and NONCE pair returned by a long-term challenge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Challenge {
    pub realm: String,
    pub nonce: String,
}

/// Key used to sign requests and verify responses.
///
/// Long-term credentials return `None` until a challenge supplies the realm.
pub fn signing_key(
    auth: &PreparedAuth,
    challenge: Option<&Challenge>,
) -> Result<Option<Vec<u8>>, String> {
    match auth.mode {
        AuthMode::Disabled => Ok(None),
        AuthMode::ShortTerm => {
            let password = auth
                .password
                .as_deref()
                .expect("prepared credentials are validated");
            integrity::short_term_key(password)
                .map(Some)
                .map_err(|e| format!("failed to derive the short-term key: {e}"))
        }
        AuthMode::LongTerm => match challenge {
            None => Ok(None),
            Some(challenge) => {
                let username = auth
                    .username
                    .as_deref()
                    .expect("prepared credentials are validated");
                let password = auth
                    .password
                    .as_deref()
                    .expect("prepared credentials are validated");
                let realm = auth.prepared_realm(&challenge.realm)?;
                integrity::long_term_key(username, &realm, password)
                    .map(|key| Some(key.to_vec()))
                    .map_err(|e| format!("failed to derive the long-term key: {e}"))
            }
        },
    }
}

/// Build a Binding Request, optionally signed and terminated by FINGERPRINT.
pub fn build_request(
    transaction_id: [u8; 12],
    fingerprint: bool,
    auth: &PreparedAuth,
    challenge: Option<&Challenge>,
) -> Result<(StunMessage, Vec<u8>), String> {
    let mut request = StunMessage::new(StunMessageType::Request, transaction_id);

    match auth.mode {
        AuthMode::Disabled => {}
        AuthMode::ShortTerm => {
            request.add_attribute(StunAttribute::new(
                StunAttributeType::Username,
                auth.username
                    .clone()
                    .expect("prepared credentials are validated")
                    .into_bytes(),
            ));
        }
        AuthMode::LongTerm => {
            if let Some(challenge) = challenge {
                let realm = auth.prepared_realm(&challenge.realm)?;
                request.add_attribute(StunAttribute::new(
                    StunAttributeType::Username,
                    auth.username
                        .clone()
                        .expect("prepared credentials are validated")
                        .into_bytes(),
                ));
                request.add_attribute(StunAttribute::new(
                    StunAttributeType::Realm,
                    realm.into_bytes(),
                ));
                request.add_attribute(StunAttribute::new(
                    StunAttributeType::Nonce,
                    challenge.nonce.as_bytes().to_vec(),
                ));
            }
        }
    }

    let key = signing_key(auth, challenge)?;
    if key.is_some() {
        request.add_attribute(StunAttribute::new(
            StunAttributeType::MessageIntegrity,
            vec![0u8; integrity::MESSAGE_INTEGRITY_LEN],
        ));
    }
    if fingerprint {
        request.add_attribute(StunAttribute::new(
            StunAttributeType::Fingerprint,
            vec![0u8; integrity::FINGERPRINT_LEN],
        ));
    }

    let mut bytes = request.encode();
    if let Some(key) = key.as_deref() {
        let mi_offset = request
            .attribute_offset(StunAttributeType::MessageIntegrity as u16)
            .expect("MESSAGE-INTEGRITY was just added");
        let digest = integrity::hmac_sha1(key, &integrity::hmac_input(&bytes, mi_offset));
        bytes[mi_offset + 4..mi_offset + 4 + integrity::MESSAGE_INTEGRITY_LEN]
            .copy_from_slice(&digest);
    }
    if fingerprint {
        let fp_offset = request
            .attribute_offset(StunAttributeType::Fingerprint as u16)
            .expect("FINGERPRINT was just added");
        let crc = integrity::fingerprint(&bytes[..fp_offset]);
        bytes[fp_offset + 4..fp_offset + 4 + integrity::FINGERPRINT_LEN]
            .copy_from_slice(&crc.to_be_bytes());
    }

    Ok((request, bytes))
}

/// Run a benchmark against the specified STUN server.
///
/// Spawns sender tasks that emit requests at the target QPS rate,
/// and a receiver task that collects responses and measures latency.
/// Returns final benchmark statistics.
pub async fn run_benchmark(config: &BenchConfig) -> Result<BenchmarkStats, String> {
    if config.server_addrs.is_empty() {
        return Err("no server addresses provided".to_string());
    }

    let auth = config.auth.prepare()?;

    // Bind a UDP socket for sending/receiving.
    let local_addr: SocketAddr = if config.server_addrs[0].is_ipv4() {
        "0.0.0.0:0".parse().unwrap()
    } else {
        "[::]:0".parse().unwrap()
    };

    let socket = Arc::new(
        bind_udp_socket(local_addr, UDP_RECV_BUFFER_SIZE, UDP_SEND_BUFFER_SIZE)
            .map_err(|e| format!("failed to bind socket: {}", e))?,
    );
    log::debug!(
        "UDP socket buffers requested: recv={} send={} (kernel caps at net.core.rmem_max/wmem_max)",
        UDP_RECV_BUFFER_SIZE,
        UDP_SEND_BUFFER_SIZE
    );

    // RFC 5389 section 10.2.1: long-term credentials first need a realm and
    // nonce from the server before requests can be signed.
    let challenge = match auth.mode() {
        AuthMode::LongTerm => {
            let challenge =
                fetch_challenge(&socket, config.server_addrs[0], CHALLENGE_TIMEOUT).await?;
            log::info!("Long-term challenge received: realm={}", challenge.realm);
            Some(challenge)
        }
        AuthMode::Disabled | AuthMode::ShortTerm => None,
    };
    let response_key = signing_key(&auth, challenge.as_ref())?.map(Arc::<[u8]>::from);
    if auth.mode() != AuthMode::Disabled {
        log::info!("Authentication mode: {}", auth.mode().as_str());
    }

    // The receiver refreshes this state when the server rejects the nonce; the
    // sender reads it for every request.
    let auth_state = Arc::new(Mutex::new(AuthState {
        challenge,
        response_key,
    }));

    // Shared state for coordination between sender and receiver.
    let total_sent = Arc::new(AtomicU64::new(0));
    let total_success = Arc::new(AtomicU64::new(0));
    let total_failed = Arc::new(AtomicU64::new(0));
    let pending: PendingMap = Arc::new(Mutex::new(HashMap::new()));
    let running = Arc::new(AtomicBool::new(true));
    let current_addr_idx = Arc::new(AtomicU64::new(0));

    // Determine test termination condition.
    let (stop_signal, stop_handle) = match (config.duration, config.request_count) {
        (Some(duration), _) => {
            // Time-based termination.
            let running_clone = running.clone();
            let handle = tokio::spawn(async move {
                tokio::time::sleep(duration).await;
                running_clone.store(false, Ordering::Relaxed);
            });
            (None, Some(handle))
        }
        (_, Some(count)) => {
            // Count-based termination (checked in sender loop).
            (Some(count), None)
        }
        _ => return Err("either duration or request_count must be specified".to_string()),
    };

    // Spawn sender task.
    let sender_socket = socket.clone();
    let sender_sent = total_sent.clone();
    let sender_failed = total_failed.clone();
    let sender_running = running.clone();
    let sender_pending = pending.clone();
    let sender_target_qps = config.target_qps;
    let sender_stop_signal = stop_signal;
    let sender_server_addrs = config.server_addrs.clone();
    let sender_addr_idx = current_addr_idx.clone();
    let sender_fingerprint = config.fingerprint;
    let sender_auth = auth.clone();
    let sender_auth_state = auth_state.clone();
    let sender_handle = tokio::spawn(async move {
        let interval_duration = if sender_target_qps > 0 {
            Duration::from_micros(1_000_000 / sender_target_qps as u64)
        } else {
            Duration::from_millis(1)
        };
        let mut interval = tokio::time::interval(interval_duration);
        let mut consecutive_failures = 0u32;
        let mut last_prune = Instant::now();

        loop {
            interval.tick().await;

            // Check termination conditions.
            if !sender_running.load(Ordering::Relaxed) {
                break;
            }
            if let Some(max_count) = sender_stop_signal {
                if sender_sent.load(Ordering::Relaxed) >= max_count {
                    sender_running.store(false, Ordering::Relaxed);
                    break;
                }
            }

            // Get current server address.
            let idx = sender_addr_idx.load(Ordering::Relaxed) as usize;
            let server_addr = sender_server_addrs[idx % sender_server_addrs.len()];

            // Build and send STUN request.
            let transaction_id = generate_transaction_id();
            let challenge = sender_auth_state
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .challenge
                .clone();
            let (request, request_bytes) = match build_request(
                transaction_id,
                sender_fingerprint,
                &sender_auth,
                challenge.as_ref(),
            ) {
                Ok(value) => value,
                Err(e) => {
                    log::error!("Failed to build request: {}", e);
                    sender_running.store(false, Ordering::Relaxed);
                    break;
                }
            };

            log::debug!("Sending: {}", describe_message(&request));
            log::trace!("Sending: {:02x?}", request_bytes);

            let sent_at = Instant::now();
            sender_pending
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .insert(transaction_id, sent_at);

            match sender_socket.send_to(&request_bytes, server_addr).await {
                Ok(_) => {
                    sender_sent.fetch_add(1, Ordering::Relaxed);
                    consecutive_failures = 0;
                }
                Err(e) => {
                    log::debug!("Failed to send to {}: {}", server_addr, e);
                    sender_pending
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .remove(&transaction_id);
                    sender_sent.fetch_add(1, Ordering::Relaxed);
                    sender_failed.fetch_add(1, Ordering::Relaxed);
                    consecutive_failures += 1;
                    // Try next address after 3 consecutive failures.
                    if consecutive_failures >= 3 && sender_server_addrs.len() > 1 {
                        let new_idx = (idx + 1) % sender_server_addrs.len();
                        sender_addr_idx.store(new_idx as u64, Ordering::Relaxed);
                        log::info!(
                            "Switching to address {} after {} failures",
                            sender_server_addrs[new_idx],
                            consecutive_failures
                        );
                        consecutive_failures = 0;
                    }
                }
            }

            // Bound memory when the server drops requests.
            if last_prune.elapsed() >= Duration::from_secs(1) {
                let stale = prune_stale(&sender_pending, PENDING_TTL);
                if stale > 0 {
                    sender_failed.fetch_add(stale, Ordering::Relaxed);
                }
                last_prune = Instant::now();
            }
        }
    });

    // Spawn receiver task.
    let receiver_socket = socket.clone();
    let receiver_success = total_success.clone();
    let receiver_failed = total_failed.clone();
    let receiver_pending = pending.clone();
    let receiver_running = running.clone();
    let receiver_auth = auth.clone();
    let receiver_auth_state = auth_state.clone();
    let receiver_capacity = expected_sample_capacity(config);
    let receiver_handle = tokio::spawn(async move {
        let mut latencies = LatencyCollector::with_capacity(receiver_capacity);
        let mut buf = vec![0u8; 65536];
        let recv_timeout = Duration::from_secs(2);

        loop {
            if !receiver_running.load(Ordering::Relaxed) {
                // Drain any remaining responses.
                match tokio::time::timeout(
                    Duration::from_millis(100),
                    receiver_socket.recv_from(&mut buf),
                )
                .await
                {
                    Ok(Ok((len, _))) => {
                        record_response(
                            &buf[..len],
                            &receiver_pending,
                            &receiver_auth,
                            &receiver_auth_state,
                            &mut latencies,
                            &receiver_success,
                            &receiver_failed,
                        );
                    }
                    _ => break,
                }
                continue;
            }

            match tokio::time::timeout(recv_timeout, receiver_socket.recv_from(&mut buf)).await {
                Ok(Ok((len, _))) => {
                    record_response(
                        &buf[..len],
                        &receiver_pending,
                        &receiver_auth,
                        &receiver_auth_state,
                        &mut latencies,
                        &receiver_success,
                        &receiver_failed,
                    );
                }
                Ok(Err(_)) => {
                    receiver_failed.fetch_add(1, Ordering::Relaxed);
                }
                Err(_) => {
                    // Timeout, check if we should stop.
                    if !receiver_running.load(Ordering::Relaxed) {
                        break;
                    }
                }
            }
        }

        latencies
    });

    // Spawn progress reporter task.
    let progress_sent = total_sent.clone();
    let progress_success = total_success.clone();
    let progress_failed = total_failed.clone();
    let progress_running = running.clone();
    let progress_handle = tokio::spawn(async move {
        // Poll the stop flag once per second while keeping the 10-second
        // report cadence, so a finished run does not linger waiting for a tick.
        let mut tick = tokio::time::interval(Duration::from_secs(1));
        let report_every = Duration::from_secs(10);
        let start = Instant::now();
        let mut next_report = Duration::ZERO;

        loop {
            tick.tick().await;
            if !progress_running.load(Ordering::Relaxed) {
                break;
            }
            let elapsed = start.elapsed();
            if elapsed < next_report {
                continue;
            }
            next_report += report_every;
            let elapsed = elapsed.as_secs();
            let sent = progress_sent.load(Ordering::Relaxed);
            let success = progress_success.load(Ordering::Relaxed);
            let failed = progress_failed.load(Ordering::Relaxed);
            let current_qps = sent.checked_div(elapsed.max(1)).unwrap_or(0);
            print_progress(elapsed, sent, success, failed, current_qps);
        }
    });

    // Wait for sender to finish.
    let _ = sender_handle.await;

    // Give the receiver a moment to process remaining responses.
    tokio::time::sleep(Duration::from_millis(500)).await;
    running.store(false, Ordering::Relaxed);

    // Wait for receiver and progress reporter.
    let latencies = receiver_handle
        .await
        .map_err(|e| format!("receiver task failed: {}", e))?;
    let _ = progress_handle.await;
    if let Some(h) = stop_handle {
        h.abort();
    }

    // Requests that never received a matching response are failures.
    let unanswered = pending.lock().unwrap_or_else(|e| e.into_inner()).len() as u64;
    if unanswered > 0 {
        total_failed.fetch_add(unanswered, Ordering::Relaxed);
    }

    // Calculate final statistics.
    let total_requests = total_sent.load(Ordering::Relaxed);
    let successful_requests = total_success.load(Ordering::Relaxed);
    let failed_requests = total_failed.load(Ordering::Relaxed);
    let latencies_vec: Vec<Duration> = latencies.latencies().to_vec();

    let duration = config.duration.unwrap_or_else(|| {
        // Estimate duration from request count and QPS.
        Duration::from_secs_f64(total_requests as f64 / config.target_qps as f64)
    });

    // Final progress line so the last report matches the summary below even
    // when the run ends before the next 10-second tick.
    let elapsed_secs = duration.as_secs();
    print_progress(
        elapsed_secs,
        total_requests,
        successful_requests,
        failed_requests,
        total_requests / elapsed_secs.max(1),
    );

    Ok(BenchmarkStats::calculate(
        config.target_qps,
        total_requests,
        successful_requests,
        failed_requests,
        &latencies_vec,
        duration,
    ))
}

/// Outcome of a received datagram.
#[derive(Debug, PartialEq, Eq)]
enum ResponseResult {
    /// A validated Binding Success Response, with its round-trip latency.
    Success(Duration),
    /// The server rejected the nonce; the challenge must be refreshed.
    StaleNonce(Challenge),
    /// A response that belongs to an outstanding request but is unusable.
    Failure,
    /// A datagram that does not match any outstanding transaction.
    Ignored,
}

/// Long-term challenge state shared between the sender and receiver.
///
/// The receiver refreshes it when the server reports a stale nonce; the sender
/// reads the current challenge for every request.
#[derive(Debug, Clone)]
struct AuthState {
    challenge: Option<Challenge>,
    response_key: Option<Arc<[u8]>>,
}

/// Adopt a refreshed challenge supplied by a 438 Stale Nonce response.
fn apply_challenge(auth: &PreparedAuth, state: &Mutex<AuthState>, challenge: Challenge) {
    if auth.mode() != AuthMode::LongTerm {
        log::debug!(
            "Ignoring a stale nonce response in {} mode",
            auth.mode().as_str()
        );
        return;
    }

    match signing_key(auth, Some(&challenge)) {
        Ok(Some(key)) => {
            log::info!(
                "Server returned 438 Stale Nonce; using nonce {}",
                challenge.nonce
            );
            let mut state = state.lock().unwrap_or_else(|e| e.into_inner());
            state.challenge = Some(challenge);
            state.response_key = Some(Arc::from(key));
        }
        Ok(None) => {}
        Err(e) => log::error!("Failed to derive a key for the refreshed nonce: {}", e),
    }
}

/// Match a response to its request and measure the round-trip latency.
///
/// Only Binding Success Responses whose transaction ID is outstanding count as
/// successes. When authentication is enabled the response must also carry a
/// valid MESSAGE-INTEGRITY (RFC 5389 sections 10.1.2 and 10.2.2).
fn process_response(
    data: &[u8],
    pending: &Mutex<HashMap<[u8; 12], Instant>>,
    expected_key: Option<&[u8]>,
) -> ResponseResult {
    log::trace!("Received: {:02x?}", data);

    let response = match StunMessage::decode(data) {
        Ok(response) => response,
        Err(_) => {
            log::debug!("Failed to decode STUN response ({} bytes)", data.len());
            return ResponseResult::Failure;
        }
    };

    let sent_at = match pending
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .remove(&response.transaction_id)
    {
        Some(sent_at) => sent_at,
        None => {
            log::debug!("Ignoring response with an unknown transaction ID");
            return ResponseResult::Ignored;
        }
    };

    match response.message_type {
        StunMessageType::Response => {}
        StunMessageType::ErrorResponse => {
            return match stale_nonce(&response) {
                Some(challenge) => {
                    log::debug!("Received 438 Stale Nonce; refreshing the challenge");
                    ResponseResult::StaleNonce(challenge)
                }
                None => {
                    log::debug!("Received STUN error response");
                    ResponseResult::Failure
                }
            };
        }
        _ => {
            log::debug!(
                "Received unusable STUN response type: {:?}",
                response.message_type
            );
            return ResponseResult::Failure;
        }
    }

    if let Some(key) = expected_key {
        let has_integrity =
            response.attribute_count(StunAttributeType::MessageIntegrity as u16) > 0;
        if !has_integrity || !StunMessage::verify_message_integrity(data, key) {
            log::debug!("Response MESSAGE-INTEGRITY missing or invalid");
            return ResponseResult::Failure;
        }
    }

    log::debug!("Received: {}", describe_message(&response));
    ResponseResult::Success(sent_at.elapsed())
}

/// Record a datagram in the counters and latency collector.
fn record_response(
    data: &[u8],
    pending: &Mutex<HashMap<[u8; 12], Instant>>,
    auth: &PreparedAuth,
    auth_state: &Mutex<AuthState>,
    latencies: &mut LatencyCollector,
    success: &AtomicU64,
    failed: &AtomicU64,
) {
    let expected_key = auth_state
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .response_key
        .clone();
    match process_response(data, pending, expected_key.as_deref()) {
        ResponseResult::Success(latency) => {
            latencies.record_latency(latency);
            success.fetch_add(1, Ordering::Relaxed);
        }
        ResponseResult::StaleNonce(challenge) => {
            // RFC 5389 section 10.2.3: the rejected request fails and the
            // following requests sign with the refreshed nonce.
            failed.fetch_add(1, Ordering::Relaxed);
            apply_challenge(auth, auth_state, challenge);
        }
        ResponseResult::Failure => {
            failed.fetch_add(1, Ordering::Relaxed);
        }
        ResponseResult::Ignored => {}
    }
}

/// Drop pending entries older than `max_age`, returning how many were removed.
fn prune_stale(pending: &Mutex<HashMap<[u8; 12], Instant>>, max_age: Duration) -> u64 {
    let now = Instant::now();
    let mut pending = pending.lock().unwrap_or_else(|e| e.into_inner());
    let before = pending.len() as u64;
    pending.retain(|_, sent_at| now.saturating_duration_since(*sent_at) <= max_age);
    before - pending.len() as u64
}

/// Extract REALM and NONCE from a 438 Stale Nonce error response.
fn stale_nonce(response: &StunMessage) -> Option<Challenge> {
    let code = response
        .attributes
        .iter()
        .find(|attribute| attribute.attribute_type == StunAttributeType::ErrorCode)
        .and_then(|attribute| ErrorCode::decode_value(&attribute.attribute_value).ok())?;
    if !(code.class == 4 && code.number == 38) {
        return None;
    }
    Some(Challenge {
        realm: text_attribute(response, StunAttributeType::Realm)?,
        nonce: text_attribute(response, StunAttributeType::Nonce)?,
    })
}

/// Read a textual attribute value.
fn text_attribute(message: &StunMessage, attribute_type: StunAttributeType) -> Option<String> {
    message
        .attributes
        .iter()
        .find(|attribute| attribute.attribute_type == attribute_type)
        .and_then(|attribute| String::from_utf8(attribute.attribute_value.clone()).ok())
}

/// Obtain REALM and NONCE from a long-term 401/438 challenge.
///
/// The bootstrap request is intentionally unauthenticated (RFC 5389 section
/// 10.2.1.1). Retries use a fresh transaction ID.
async fn fetch_challenge(
    socket: &UdpSocket,
    server_addr: SocketAddr,
    timeout: Duration,
) -> Result<Challenge, String> {
    let mut buf = vec![0u8; 65536];

    for _ in 0..CHALLENGE_ATTEMPTS {
        let transaction_id = generate_transaction_id();
        let request = StunMessage::new(StunMessageType::Request, transaction_id);
        socket
            .send_to(&request.encode(), server_addr)
            .await
            .map_err(|e| format!("failed to send challenge request: {}", e))?;

        let (len, _) = match tokio::time::timeout(timeout, socket.recv_from(&mut buf)).await {
            Ok(Ok(value)) => value,
            Ok(Err(e)) => return Err(format!("failed to receive challenge: {}", e)),
            Err(_) => continue,
        };

        let response = match StunMessage::decode(&buf[..len]) {
            Ok(response) => response,
            Err(_) => continue,
        };
        if response.transaction_id != transaction_id
            || response.message_type != StunMessageType::ErrorResponse
        {
            continue;
        }

        let code = response
            .attributes
            .iter()
            .find(|attribute| attribute.attribute_type == StunAttributeType::ErrorCode)
            .and_then(|attribute| ErrorCode::decode_value(&attribute.attribute_value).ok());
        let Some(code) = code else {
            continue;
        };
        if !(code.class == 4 && (code.number == 1 || code.number == 38)) {
            continue;
        }

        let realm = text_attribute(&response, StunAttributeType::Realm);
        let nonce = text_attribute(&response, StunAttributeType::Nonce);
        if let (Some(realm), Some(nonce)) = (realm, nonce) {
            return Ok(Challenge { realm, nonce });
        }
    }

    Err(format!(
        "server {} did not return a 401 or 438 challenge with REALM and NONCE",
        server_addr
    ))
}

/// Print real-time progress update.
fn print_progress(elapsed_secs: u64, sent: u64, success: u64, failed: u64, current_qps: u64) {
    let minutes = elapsed_secs / 60;
    let seconds = elapsed_secs % 60;
    println!(
        "[{:02}:{:02}] Sent: {}, Success: {}, Failed: {}, QPS: {}",
        minutes, seconds, sent, success, failed, current_qps
    );
}

/// Print final benchmark results.
pub fn print_results(stats: &BenchmarkStats) {
    println!("\nBenchmark Complete");
    println!("==================");
    println!("Duration: {:.1}s", stats.duration.as_secs_f64());
    println!("Target QPS: {}", stats.target_qps);
    println!("Actual QPS: {:.2}", stats.actual_qps);
    println!("Total Requests: {}", stats.total_requests);
    println!(
        "Successful: {} ({:.2}%)",
        stats.successful_requests, stats.success_rate
    );
    println!("Failed: {}", stats.failed_requests);
    println!("\nLatency Percentiles:");
    println!("  p50: {:.2}ms", stats.latency_p50.as_secs_f64() * 1000.0);
    println!("  p95: {:.2}ms", stats.latency_p95.as_secs_f64() * 1000.0);
    println!("  p99: {:.2}ms", stats.latency_p99.as_secs_f64() * 1000.0);
}

/// Generate a random 12-byte transaction ID.
fn generate_transaction_id() -> [u8; 12] {
    let mut id = [0u8; 12];
    rand::rngs::OsRng.fill_bytes(&mut id);
    id
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expected_sample_capacity_prefers_request_count() {
        let config = BenchConfig {
            server_addrs: vec!["127.0.0.1:3478".parse().unwrap()],
            target_qps: 1_000,
            duration: Some(Duration::from_secs(10)),
            request_count: Some(1_234),
            fingerprint: false,
            auth: BenchAuth::disabled(),
        };

        assert_eq!(expected_sample_capacity(&config), 1_234);
    }

    #[test]
    fn test_expected_sample_capacity_estimates_duration_runs() {
        let config = BenchConfig {
            server_addrs: vec!["127.0.0.1:3478".parse().unwrap()],
            target_qps: 50_000,
            duration: Some(Duration::from_secs(60)),
            request_count: None,
            fingerprint: false,
            auth: BenchAuth::disabled(),
        };

        assert_eq!(expected_sample_capacity(&config), 3_000_000);
    }

    #[test]
    fn test_expected_sample_capacity_is_capped() {
        let config = BenchConfig {
            server_addrs: vec!["127.0.0.1:3478".parse().unwrap()],
            target_qps: u32::MAX,
            duration: Some(Duration::from_secs(86_400)),
            request_count: None,
            fingerprint: false,
            auth: BenchAuth::disabled(),
        };

        assert_eq!(expected_sample_capacity(&config), MAX_PREALLOCATED_SAMPLES);
    }

    #[tokio::test]
    async fn test_bind_udp_socket_applies_buffer_sizes() {
        let plain = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
        let plain = socket2::SockRef::from(&plain);
        let default_recv = plain.recv_buffer_size().unwrap();
        let default_send = plain.send_buffer_size().unwrap();

        let socket = bind_udp_socket(
            "127.0.0.1:0".parse().unwrap(),
            UDP_RECV_BUFFER_SIZE,
            UDP_SEND_BUFFER_SIZE,
        )
        .unwrap();
        let std_socket = socket.into_std().unwrap();
        let socket = socket2::SockRef::from(&std_socket);

        // The kernel caps the request at net.core.rmem_max/wmem_max, but a
        // tuned socket must still exceed the system default.
        assert!(socket.recv_buffer_size().unwrap() > default_recv);
        assert!(socket.send_buffer_size().unwrap() > default_send);
    }

    #[test]
    fn test_bench_config_creation() {
        let config = BenchConfig {
            server_addrs: vec!["127.0.0.1:3478".parse().unwrap()],
            target_qps: 1000,
            duration: Some(Duration::from_secs(60)),
            request_count: None,
            fingerprint: false,
            auth: BenchAuth::disabled(),
        };
        assert_eq!(config.target_qps, 1000);
        assert!(config.duration.is_some());
        assert!(config.request_count.is_none());
    }

    #[test]
    fn test_generate_transaction_id() {
        let id1 = generate_transaction_id();
        let id2 = generate_transaction_id();
        assert_ne!(id1, id2);
        assert_eq!(id1.len(), 12);
    }

    #[test]
    fn test_prune_stale_removes_only_expired_entries() {
        let pending = Mutex::new(HashMap::new());
        let now = Instant::now();
        pending
            .lock()
            .unwrap()
            .insert([1; 12], now.checked_sub(Duration::from_secs(10)).unwrap());
        pending.lock().unwrap().insert([2; 12], now);

        let removed = prune_stale(&pending, PENDING_TTL);

        assert_eq!(removed, 1);
        assert!(!pending.lock().unwrap().contains_key(&[1; 12]));
        assert!(pending.lock().unwrap().contains_key(&[2; 12]));
    }

    #[tokio::test]
    async fn test_run_benchmark_invalid_config() {
        let config = BenchConfig {
            server_addrs: vec![],
            target_qps: 100,
            duration: None,
            request_count: None,
            fingerprint: false,
            auth: BenchAuth::disabled(),
        };
        let result = run_benchmark(&config).await;
        assert!(result.is_err());
    }

    #[test]
    fn test_process_response_reports_stale_nonce() {
        let pending = Mutex::new(HashMap::new());
        pending.lock().unwrap().insert([7; 12], Instant::now());

        let mut response = StunMessage::new(StunMessageType::ErrorResponse, [7; 12]);
        let code = ErrorCode::new(4, 38, "Stale Nonce".to_string()).unwrap();
        response.add_attribute(StunAttribute::new(
            StunAttributeType::ErrorCode,
            code.encode_value(),
        ));
        response.add_attribute(StunAttribute::new(
            StunAttributeType::Realm,
            b"example.org".to_vec(),
        ));
        response.add_attribute(StunAttribute::new(
            StunAttributeType::Nonce,
            b"nonce-2".to_vec(),
        ));

        let result = process_response(&response.encode(), &pending, None);

        assert_eq!(
            result,
            ResponseResult::StaleNonce(Challenge {
                realm: "example.org".to_string(),
                nonce: "nonce-2".to_string(),
            })
        );
        assert!(pending.lock().unwrap().is_empty());
    }

    #[test]
    fn test_process_response_unauthorized_is_not_stale_nonce() {
        let pending = Mutex::new(HashMap::new());
        pending.lock().unwrap().insert([8; 12], Instant::now());

        let mut response = StunMessage::new(StunMessageType::ErrorResponse, [8; 12]);
        let code = ErrorCode::new(4, 1, "Unauthorized".to_string()).unwrap();
        response.add_attribute(StunAttribute::new(
            StunAttributeType::ErrorCode,
            code.encode_value(),
        ));
        response.add_attribute(StunAttribute::new(
            StunAttributeType::Realm,
            b"example.org".to_vec(),
        ));
        response.add_attribute(StunAttribute::new(
            StunAttributeType::Nonce,
            b"nonce-2".to_vec(),
        ));

        assert_eq!(
            process_response(&response.encode(), &pending, None),
            ResponseResult::Failure
        );
    }

    #[test]
    fn test_long_term_realm_override_changes_attribute_and_key() {
        let auth = BenchAuth::long_term_with_realm("alice", "s3cret", "override.example")
            .prepare()
            .unwrap();
        let challenge = Challenge {
            realm: "example.org".to_string(),
            nonce: "nonce-1".to_string(),
        };

        let (message, bytes) = build_request([0x42; 12], false, &auth, Some(&challenge)).unwrap();

        assert!(has_text(
            &message,
            StunAttributeType::Realm,
            "override.example"
        ));
        assert!(has_text(&message, StunAttributeType::Nonce, "nonce-1"));
        let override_key = integrity::long_term_key("alice", "override.example", "s3cret").unwrap();
        let challenge_key = integrity::long_term_key("alice", "example.org", "s3cret").unwrap();
        assert!(StunMessage::verify_message_integrity(&bytes, &override_key));
        assert!(!StunMessage::verify_message_integrity(
            &bytes,
            &challenge_key
        ));

        let key = signing_key(&auth, Some(&challenge)).unwrap().unwrap();
        assert_eq!(key, override_key);
    }

    fn has_text(message: &StunMessage, attribute_type: StunAttributeType, value: &str) -> bool {
        message.attributes.iter().any(|attribute| {
            attribute.attribute_type == attribute_type
                && attribute.attribute_value == value.as_bytes()
        })
    }

    #[test]
    fn test_apply_challenge_updates_signing_state() {
        let auth = BenchAuth::long_term("alice", "s3cret").prepare().unwrap();
        let old = Challenge {
            realm: "example.org".to_string(),
            nonce: "nonce-1".to_string(),
        };
        let old_key = signing_key(&auth, Some(&old)).unwrap().unwrap();
        let state = Mutex::new(AuthState {
            challenge: Some(old),
            response_key: Some(Arc::from(old_key)),
        });

        apply_challenge(
            &auth,
            &state,
            Challenge {
                realm: "example.org".to_string(),
                nonce: "nonce-2".to_string(),
            },
        );

        let state = state.lock().unwrap();
        assert_eq!(state.challenge.as_ref().unwrap().nonce, "nonce-2");
        let expected = integrity::long_term_key("alice", "example.org", "s3cret").unwrap();
        assert_eq!(state.response_key.as_deref().unwrap(), expected.as_slice());
    }

    #[test]
    fn test_apply_challenge_keeps_realm_override() {
        let auth = BenchAuth::long_term_with_realm("alice", "s3cret", "override.example")
            .prepare()
            .unwrap();
        let initial = Challenge {
            realm: "example.org".to_string(),
            nonce: "nonce-1".to_string(),
        };
        let initial_key = signing_key(&auth, Some(&initial)).unwrap().unwrap();
        let state = Mutex::new(AuthState {
            challenge: Some(initial),
            response_key: Some(Arc::from(initial_key)),
        });

        apply_challenge(
            &auth,
            &state,
            Challenge {
                realm: "rotated.example".to_string(),
                nonce: "nonce-2".to_string(),
            },
        );

        let state = state.lock().unwrap();
        assert_eq!(state.challenge.as_ref().unwrap().nonce, "nonce-2");
        let expected = integrity::long_term_key("alice", "override.example", "s3cret").unwrap();
        assert_eq!(state.response_key.as_deref().unwrap(), expected.as_slice());
    }

    #[test]
    fn test_print_progress() {
        // Just verify it doesn't panic.
        print_progress(10, 1000, 995, 5, 100);
    }

    #[test]
    fn test_print_results() {
        let stats = BenchmarkStats {
            target_qps: 1000,
            actual_qps: 995.5,
            total_requests: 10000,
            successful_requests: 9950,
            failed_requests: 50,
            success_rate: 99.5,
            latency_p50: Duration::from_millis(1),
            latency_p95: Duration::from_millis(2),
            latency_p99: Duration::from_millis(5),
            duration: Duration::from_secs(10),
        };
        // Just verify it doesn't panic.
        print_results(&stats);
    }
}
