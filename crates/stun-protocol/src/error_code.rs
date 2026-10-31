//! ERROR-CODE and UNKNOWN-ATTRIBUTES attribute value codecs.
//!
//! See RFC 5389 sections 15.6 and 15.9.

use crate::error::StunError;
use byteorder::{BigEndian, ByteOrder};

/// Decoded ERROR-CODE attribute value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErrorCode {
    /// Error class (3 bits on the wire, values 0-7).
    pub class: u8,
    /// Error number (8 bits).
    pub number: u8,
    /// Human-readable reason phrase (UTF-8).
    pub reason: String,
}

impl ErrorCode {
    /// Create an error code, masking the class to three bits.
    pub fn new(class: u8, number: u8, reason: String) -> Self {
        Self {
            class: class & 0x07,
            number,
            reason,
        }
    }

    /// Combined three-digit status code (for example 420).
    pub fn status_code(&self) -> u16 {
        u16::from(self.class) * 100 + u16::from(self.number)
    }

    /// Encode the attribute value (4-byte header plus reason, without padding).
    pub fn encode_value(&self) -> Vec<u8> {
        let mut buffer = Vec::with_capacity(4 + self.reason.len());
        buffer.extend_from_slice(&[0x00, 0x00, self.class & 0x07, self.number]);
        buffer.extend_from_slice(self.reason.as_bytes());
        buffer
    }

    /// Decode an attribute value. Padding is handled by the attribute layer and
    /// must not be included here.
    pub fn decode_value(data: &[u8]) -> Result<Self, StunError> {
        if data.len() < 4 {
            return Err(StunError::MalformedMessage(
                "ERROR-CODE value too short".to_string(),
            ));
        }
        Ok(Self {
            class: data[2] & 0x07,
            number: data[3],
            reason: String::from_utf8_lossy(&data[4..]).into_owned(),
        })
    }
}

/// Decoded UNKNOWN-ATTRIBUTES attribute value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownAttributes {
    /// Attribute types the peer did not understand.
    pub types: Vec<u16>,
}

impl UnknownAttributes {
    /// Create a list from attribute types.
    pub fn new(types: Vec<u16>) -> Self {
        Self { types }
    }

    /// Encode the attribute value as big-endian types, de-duplicated in
    /// first-seen order, without padding.
    pub fn encode_value(&self) -> Vec<u8> {
        let mut seen = Vec::with_capacity(self.types.len());
        let mut buffer = Vec::with_capacity(self.types.len() * 2);
        for &attribute_type in &self.types {
            if seen.contains(&attribute_type) {
                continue;
            }
            seen.push(attribute_type);
            let mut type_bytes = [0u8; 2];
            BigEndian::write_u16(&mut type_bytes, attribute_type);
            buffer.extend_from_slice(&type_bytes);
        }
        buffer
    }

    /// Decode an attribute value. The value length must be even.
    pub fn decode_value(data: &[u8]) -> Result<Self, StunError> {
        if !data.len().is_multiple_of(2) {
            return Err(StunError::MalformedMessage(
                "UNKNOWN-ATTRIBUTES value length must be even".to_string(),
            ));
        }
        let types = data
            .as_chunks::<2>()
            .0
            .iter()
            .map(|chunk| BigEndian::read_u16(chunk))
            .collect();
        Ok(Self { types })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_code_status_code() {
        assert_eq!(ErrorCode::new(4, 20, String::new()).status_code(), 420);
        assert_eq!(ErrorCode::new(5, 0, String::new()).status_code(), 500);
    }
}
