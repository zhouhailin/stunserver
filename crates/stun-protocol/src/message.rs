//! STUN message types and structures
//!
//! This module implements STUN message parsing and serialization according to
//! RFC 5389 Section 6 (STUN Message Structure).

use crate::attributes::StunAttribute;
use crate::error::StunError;
use byteorder::{BigEndian, ByteOrder};

/// STUN message types as defined in RFC 5389 Section 6
///
/// The message type field is 2 bytes and encodes the message class and method.
/// For Binding messages:
/// - Request: 0x0001 (Section 3.1)
/// - Response: 0x0101 (Section 3.1)
/// - ErrorIndication: 0x0111 (Section 3.1)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StunMessageType {
    /// STUN Binding Request (0x0001) - RFC 5389 Section 3.1
    Request,
    /// STUN Binding Success Response (0x0101) - RFC 5389 Section 3.1
    Response,
    /// STUN Error Response (0x0111) - RFC 5389 Section 3.1
    ErrorIndication,
}

impl StunMessageType {
    /// Convert to raw u16 value
    pub fn to_raw(&self) -> u16 {
        match self {
            StunMessageType::Request => 0x0001,
            StunMessageType::Response => 0x0101,
            StunMessageType::ErrorIndication => 0x0111,
        }
    }

    /// Convert from raw u16 value
    pub fn from_raw(value: u16) -> Result<Self, StunError> {
        match value {
            0x0001 => Ok(StunMessageType::Request),
            0x0101 => Ok(StunMessageType::Response),
            0x0111 => Ok(StunMessageType::ErrorIndication),
            _ => Err(StunError::InvalidMessageType(value)),
        }
    }
}

/// STUN message structure as defined in RFC 5389 Section 6
///
/// All STUN messages start with a 20-byte header:
/// - Message Type (2 bytes): Identifies the message type and class
/// - Message Length (2 bytes): Total length of attributes (excluding header)
/// - Magic Cookie (4 bytes): Fixed value 0x2112A442 (RFC 5389 Section 6)
/// - Transaction ID (12 bytes): Unique identifier for request-response pairs
/// - Attributes (variable): Zero or more attributes
#[derive(Debug, Clone)]
pub struct StunMessage {
    /// Message type (RFC 5389 Section 6, bytes 0-1)
    pub message_type: StunMessageType,
    /// Message length in bytes, excluding 20-byte header (RFC 5389 Section 6, bytes 2-3)
    pub message_length: u16,
    /// Magic cookie - must be 0x2112A442 (RFC 5389 Section 6, bytes 4-7)
    pub magic_cookie: u32,
    /// Transaction ID - 96-bit identifier (RFC 5389 Section 6, bytes 8-19)
    pub transaction_id: [u8; 12],
    /// Message attributes (RFC 5389 Section 6, bytes 20+)
    pub attributes: Vec<StunAttribute>,
}

/// Magic cookie value as defined in RFC 5389 Section 6
///
/// The magic cookie field MUST contain the fixed value 0x2112A442.
/// This value was chosen to be distinct from the random transaction ID
/// space and to help distinguish STUN messages from other protocols.
pub const MAGIC_COOKIE: u32 = 0x2112A442;

/// STUN header size in bytes (RFC 5389 Section 6)
///
/// All STUN messages MUST be exactly 20 bytes header + attributes.
pub const HEADER_SIZE: usize = 20;

impl StunMessage {
    /// Create a new STUN message
    pub fn new(message_type: StunMessageType, transaction_id: [u8; 12]) -> Self {
        Self {
            message_type,
            message_length: 0,
            magic_cookie: MAGIC_COOKIE,
            transaction_id,
            attributes: Vec::new(),
        }
    }

    /// Add an attribute to the message
    pub fn add_attribute(&mut self, attr: StunAttribute) {
        self.attributes.push(attr);
        self.update_length();
    }

    /// Update message length based on attributes
    fn update_length(&mut self) {
        let total_attr_len: usize = self.attributes.iter().map(|attr| attr.encoded_size()).sum();
        self.message_length = total_attr_len as u16;
    }

    /// Encode message to byte buffer
    pub fn encode(&self) -> Vec<u8> {
        let mut buffer = Vec::with_capacity(HEADER_SIZE + self.message_length as usize);

        // Encode header (20 bytes)
        // Message type (2 bytes)
        let mut type_bytes = [0u8; 2];
        BigEndian::write_u16(&mut type_bytes, self.message_type.to_raw());
        buffer.extend_from_slice(&type_bytes);

        // Message length (2 bytes)
        let mut length_bytes = [0u8; 2];
        BigEndian::write_u16(&mut length_bytes, self.message_length);
        buffer.extend_from_slice(&length_bytes);

        // Magic cookie (4 bytes)
        let mut cookie_bytes = [0u8; 4];
        BigEndian::write_u32(&mut cookie_bytes, self.magic_cookie);
        buffer.extend_from_slice(&cookie_bytes);

        // Transaction ID (12 bytes)
        buffer.extend_from_slice(&self.transaction_id);

        // Encode attributes
        for attr in &self.attributes {
            buffer.extend(attr.encode());
        }

        buffer
    }

    /// Decode message from byte buffer
    pub fn decode(data: &[u8]) -> Result<Self, StunError> {
        // Validate minimum size
        if data.len() < HEADER_SIZE {
            return Err(StunError::MalformedMessage(format!(
                "Message too short: {} bytes, minimum {}",
                data.len(),
                HEADER_SIZE
            )));
        }

        // Parse header
        let message_type_raw = BigEndian::read_u16(&data[0..2]);
        let message_type = StunMessageType::from_raw(message_type_raw)?;

        let message_length = BigEndian::read_u16(&data[2..4]);
        let magic_cookie = BigEndian::read_u32(&data[4..8]);

        // Validate magic cookie
        if magic_cookie != MAGIC_COOKIE {
            return Err(StunError::MalformedMessage(format!(
                "Invalid magic cookie: 0x{:08X}, expected 0x{:08X}",
                magic_cookie, MAGIC_COOKIE
            )));
        }

        let mut transaction_id = [0u8; 12];
        transaction_id.copy_from_slice(&data[8..20]);

        // Validate message length
        if data.len() < HEADER_SIZE + message_length as usize {
            return Err(StunError::MalformedMessage(format!(
                "Message length mismatch: header says {} bytes, but only {} bytes available",
                message_length,
                data.len() - HEADER_SIZE
            )));
        }

        // Validate message length is multiple of 4
        if message_length % 4 != 0 {
            return Err(StunError::MalformedMessage(format!(
                "Message length {} is not a multiple of 4",
                message_length
            )));
        }

        // Parse attributes
        let mut attributes = Vec::new();
        let mut offset = HEADER_SIZE;
        let end = HEADER_SIZE + message_length as usize;

        while offset < end {
            if offset + 4 > end {
                return Err(StunError::MalformedMessage(
                    "Incomplete attribute header".to_string(),
                ));
            }

            let attr = StunAttribute::decode(&data[offset..end])?;
            offset += attr.encoded_size();
            attributes.push(attr);
        }

        if offset != end {
            return Err(StunError::MalformedMessage(format!(
                "Attribute parsing ended at offset {} but message ends at {}",
                offset, end
            )));
        }

        Ok(Self {
            message_type,
            message_length,
            magic_cookie,
            transaction_id,
            attributes,
        })
    }
}
