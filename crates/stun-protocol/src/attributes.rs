//! STUN attribute types and structures

use crate::error::StunError;
use byteorder::{BigEndian, ByteOrder};

/// STUN attribute types as defined in RFC 5389 and RFC 5780
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum StunAttributeType {
    /// MAPPED-ADDRESS attribute (RFC 5389)
    MappedAddress = 0x0001,
    /// ERROR-CODE attribute (RFC 5389)
    ErrorCode = 0x0009,
    /// XOR-MAPPED-ADDRESS attribute (RFC 5389)
    XorMappedAddress = 0x0020,
    /// SOFTWARE attribute (RFC 5389)
    Software = 0x8022,
    /// SOURCE-ADDRESS attribute (RFC 5780)
    SourceAddress = 0x802B,
    /// CHANGED-ADDRESS attribute (RFC 5780)
    ChangedAddress = 0x802C,
    /// OTHER-ADDRESS attribute (RFC 5780)
    OtherAddress = 0x802D,
    /// Unknown attribute (for attributes we don't parse but can skip)
    Unknown = 0xFFFF,
}

impl StunAttributeType {
    /// Convert from raw u16 value
    pub fn from_raw(value: u16) -> Self {
        match value {
            0x0001 => StunAttributeType::MappedAddress,
            0x0009 => StunAttributeType::ErrorCode,
            0x0020 => StunAttributeType::XorMappedAddress,
            0x8022 => StunAttributeType::Software,
            0x802B => StunAttributeType::SourceAddress,
            0x802C => StunAttributeType::ChangedAddress,
            0x802D => StunAttributeType::OtherAddress,
            _ => StunAttributeType::Unknown,
        }
    }
}

/// STUN attribute structure
#[derive(Debug, Clone)]
pub struct StunAttribute {
    /// Attribute type
    pub attribute_type: StunAttributeType,
    /// Raw attribute type code as sent on the wire
    pub raw_type: u16,
    /// Attribute length
    pub attribute_length: u16,
    /// Attribute value (raw bytes)
    pub attribute_value: Vec<u8>,
}

impl StunAttribute {
    /// Create a new STUN attribute
    pub fn new(attribute_type: StunAttributeType, value: Vec<u8>) -> Self {
        let length = value.len() as u16;
        Self {
            attribute_type,
            raw_type: attribute_type as u16,
            attribute_length: length,
            attribute_value: value,
        }
    }

    /// Calculate encoded size including padding
    pub fn encoded_size(&self) -> usize {
        // 4 bytes header + value length + padding to 4-byte boundary
        let value_len = self.attribute_length as usize;
        let padding = (4 - (value_len % 4)) % 4;
        4 + value_len + padding
    }

    /// Encode attribute to byte buffer with padding
    pub fn encode(&self) -> Vec<u8> {
        let mut buffer = Vec::with_capacity(self.encoded_size());

        // Attribute type (2 bytes)
        let mut type_bytes = [0u8; 2];
        BigEndian::write_u16(&mut type_bytes, self.raw_type);
        buffer.extend_from_slice(&type_bytes);

        // Attribute length (2 bytes)
        let mut length_bytes = [0u8; 2];
        BigEndian::write_u16(&mut length_bytes, self.attribute_length);
        buffer.extend_from_slice(&length_bytes);

        // Attribute value
        buffer.extend_from_slice(&self.attribute_value);

        // Padding to 4-byte boundary
        let padding = (4 - (self.attribute_length as usize % 4)) % 4;
        buffer.extend(std::iter::repeat_n(0, padding));

        buffer
    }

    /// Decode attribute from byte buffer
    pub fn decode(data: &[u8]) -> Result<Self, StunError> {
        if data.len() < 4 {
            return Err(StunError::MalformedMessage(
                "Attribute data too short".to_string(),
            ));
        }

        let attr_type_raw = BigEndian::read_u16(&data[0..2]);
        let attribute_type = StunAttributeType::from_raw(attr_type_raw);

        let attribute_length = BigEndian::read_u16(&data[2..4]);

        let value_end = 4 + attribute_length as usize;
        if data.len() < value_end {
            return Err(StunError::MalformedMessage(format!(
                "Attribute value truncated: expected {} bytes, got {}",
                attribute_length,
                data.len() - 4
            )));
        }

        let attribute_value = data[4..value_end].to_vec();

        Ok(Self {
            attribute_type,
            raw_type: attr_type_raw,
            attribute_length,
            attribute_value,
        })
    }
}
