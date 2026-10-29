//! STUN message types and structures

use crate::attributes::StunAttribute;
use crate::error::StunError;
use byteorder::{BigEndian, ByteOrder};

/// STUN message types as defined in RFC 5389
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StunMessageType {
    /// STUN Binding Request (0x0001)
    Request,
    /// STUN Binding Success Response (0x0101)
    Response,
    /// STUN Error Response (0x0111)
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

/// STUN message structure
#[derive(Debug, Clone)]
pub struct StunMessage {
    /// Message type
    pub message_type: StunMessageType,
    /// Message length (excluding 20-byte header)
    pub message_length: u16,
    /// Magic cookie (must be 0x2112A442)
    pub magic_cookie: u32,
    /// Transaction ID (12 bytes)
    pub transaction_id: [u8; 12],
    /// Message attributes
    pub attributes: Vec<StunAttribute>,
}

/// Magic cookie value as defined in RFC 5389
pub const MAGIC_COOKIE: u32 = 0x2112A442;

/// STUN header size in bytes
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

        Ok(Self {
            message_type,
            message_length,
            magic_cookie,
            transaction_id,
            attributes,
        })
    }
}
