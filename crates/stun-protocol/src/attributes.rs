//! STUN attribute types and structures

use crate::error::StunError;
use byteorder::{BigEndian, ByteOrder};

/// STUN attribute types as defined in RFC 5389 and RFC 5780
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum StunAttributeType {
    /// MAPPED-ADDRESS attribute (RFC 5389)
    MappedAddress = 0x0001,
    /// CHANGE-REQUEST attribute (RFC 5780)
    ChangeRequest = 0x0003,
    /// USERNAME attribute (RFC 5389)
    Username = 0x0006,
    /// MESSAGE-INTEGRITY attribute (RFC 5389)
    MessageIntegrity = 0x0008,
    /// ERROR-CODE attribute (RFC 5389)
    ErrorCode = 0x0009,
    /// UNKNOWN-ATTRIBUTES attribute (RFC 5389)
    UnknownAttributes = 0x000A,
    /// REALM attribute (RFC 5389)
    Realm = 0x0014,
    /// NONCE attribute (RFC 5389)
    Nonce = 0x0015,
    /// XOR-MAPPED-ADDRESS attribute (RFC 5389)
    XorMappedAddress = 0x0020,
    /// PADDING attribute (RFC 5780)
    Padding = 0x0026,
    /// RESPONSE-PORT attribute (RFC 5780)
    ResponsePort = 0x0027,
    /// SOFTWARE attribute (RFC 5389)
    Software = 0x8022,
    /// ALTERNATE-SERVER attribute (RFC 5389)
    AlternateServer = 0x8023,
    /// CACHE-TIMEOUT attribute (RFC 5780)
    CacheTimeout = 0x8027,
    /// RESPONSE-ORIGIN attribute (RFC 5780)
    ResponseOrigin = 0x802B,
    /// OTHER-ADDRESS attribute (RFC 5780)
    OtherAddress = 0x802C,
    /// FINGERPRINT attribute (RFC 5389)
    Fingerprint = 0x8028,
    /// Unknown attribute (for attributes we don't parse but can skip)
    Unknown = 0xFFFF,
}

impl StunAttributeType {
    /// Convert from raw u16 value
    pub fn from_raw(value: u16) -> Self {
        match value {
            0x0001 => StunAttributeType::MappedAddress,
            0x0003 => StunAttributeType::ChangeRequest,
            0x0006 => StunAttributeType::Username,
            0x0008 => StunAttributeType::MessageIntegrity,
            0x0009 => StunAttributeType::ErrorCode,
            0x000A => StunAttributeType::UnknownAttributes,
            0x0014 => StunAttributeType::Realm,
            0x0015 => StunAttributeType::Nonce,
            0x0020 => StunAttributeType::XorMappedAddress,
            0x0026 => StunAttributeType::Padding,
            0x0027 => StunAttributeType::ResponsePort,
            0x8022 => StunAttributeType::Software,
            0x8023 => StunAttributeType::AlternateServer,
            0x8027 => StunAttributeType::CacheTimeout,
            0x8028 => StunAttributeType::Fingerprint,
            0x802B => StunAttributeType::ResponseOrigin,
            0x802C => StunAttributeType::OtherAddress,
            _ => StunAttributeType::Unknown,
        }
    }
}

/// Whether an attribute type is comprehension-required.
///
/// RFC 5389 section 15 divides the attribute space into a required range
/// (`0x0000-0x7FFF`) and an optional range (`0x8000-0xFFFF`).
pub fn is_comprehension_required(raw: u16) -> bool {
    raw < 0x8000
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
    /// Wire padding bytes following the value
    ///
    /// RFC 5389 section 15 allows padding to be any value; preserving it keeps
    /// decode/encode round trips byte-identical.
    pub padding: Vec<u8>,
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
            padding: Vec::new(),
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
        if self.padding.len() == padding {
            buffer.extend_from_slice(&self.padding);
        } else {
            buffer.extend(std::iter::repeat_n(0, padding));
        }

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

        // RFC 5389 sections 15.4 and 15.5 fix these value lengths.
        match (attr_type_raw, attribute_length) {
            (0x0008, len) if len != 20 => {
                return Err(StunError::MalformedMessage(format!(
                    "MESSAGE-INTEGRITY value must be 20 bytes, got {len}"
                )));
            }
            (0x8028, len) if len != 4 => {
                return Err(StunError::MalformedMessage(format!(
                    "FINGERPRINT value must be 4 bytes, got {len}"
                )));
            }
            _ => {}
        }

        let value_end = 4 + attribute_length as usize;
        if data.len() < value_end {
            return Err(StunError::MalformedMessage(format!(
                "Attribute value truncated: expected {} bytes, got {}",
                attribute_length,
                data.len() - 4
            )));
        }

        let attribute_value = data[4..value_end].to_vec();

        let padding_len = (4 - (attribute_length as usize % 4)) % 4;
        let padding_end = value_end + padding_len;
        if data.len() < padding_end {
            return Err(StunError::MalformedMessage(format!(
                "Attribute padding truncated: expected {} bytes, got {}",
                padding_len,
                data.len().saturating_sub(value_end)
            )));
        }
        let padding = data[value_end..padding_end].to_vec();

        Ok(Self {
            attribute_type,
            raw_type: attr_type_raw,
            attribute_length,
            attribute_value,
            padding,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_comprehension_required_range() {
        assert!(is_comprehension_required(0x0000));
        assert!(is_comprehension_required(0x0001));
        assert!(is_comprehension_required(0x7FFF));
        assert!(!is_comprehension_required(0x8000));
        assert!(!is_comprehension_required(0x8022));
        assert!(!is_comprehension_required(0xFFFF));
    }

    #[test]
    fn test_rfc5389_attribute_registry() {
        let expected = [
            (0x0006u16, StunAttributeType::Username),
            (0x0008, StunAttributeType::MessageIntegrity),
            (0x0014, StunAttributeType::Realm),
            (0x0015, StunAttributeType::Nonce),
            (0x8022, StunAttributeType::Software),
            (0x8023, StunAttributeType::AlternateServer),
            (0x8028, StunAttributeType::Fingerprint),
            (0x0001, StunAttributeType::MappedAddress),
            (0x0009, StunAttributeType::ErrorCode),
            (0x000A, StunAttributeType::UnknownAttributes),
            (0x0020, StunAttributeType::XorMappedAddress),
            (0x0003, StunAttributeType::ChangeRequest),
            (0x0026, StunAttributeType::Padding),
            (0x0027, StunAttributeType::ResponsePort),
            (0x8027, StunAttributeType::CacheTimeout),
            (0x802B, StunAttributeType::ResponseOrigin),
            (0x802C, StunAttributeType::OtherAddress),
        ];

        for (raw, expected_type) in expected {
            assert_eq!(StunAttributeType::from_raw(raw), expected_type);
        }
    }

    #[test]
    fn test_rfc5389_auth_attribute_wire_codes() {
        assert_eq!(StunAttributeType::Username as u16, 0x0006);
        assert_eq!(StunAttributeType::MessageIntegrity as u16, 0x0008);
        assert_eq!(StunAttributeType::Realm as u16, 0x0014);
        assert_eq!(StunAttributeType::Nonce as u16, 0x0015);
        assert_eq!(StunAttributeType::Software as u16, 0x8022);
        assert_eq!(StunAttributeType::AlternateServer as u16, 0x8023);
        assert_eq!(StunAttributeType::Fingerprint as u16, 0x8028);
    }

    #[test]
    fn test_rfc5389_registry_distinguishes_unknown() {
        assert_eq!(
            StunAttributeType::from_raw(0x0024),
            StunAttributeType::Unknown
        );
        assert_eq!(
            StunAttributeType::from_raw(0x0002),
            StunAttributeType::Unknown
        );
    }

    #[test]
    fn test_comprehension_required_boundary_is_exact() {
        assert!(is_comprehension_required(0x7FFF));
        assert!(!is_comprehension_required(0x8000));
        assert!(is_comprehension_required(0x0008));
        assert!(!is_comprehension_required(0x8028));
    }

    #[test]
    fn test_rfc5780_attribute_registry() {
        assert_eq!(
            StunAttributeType::from_raw(0x0003),
            StunAttributeType::ChangeRequest
        );
        assert_eq!(
            StunAttributeType::from_raw(0x0026),
            StunAttributeType::Padding
        );
        assert_eq!(
            StunAttributeType::from_raw(0x0027),
            StunAttributeType::ResponsePort
        );
        assert_eq!(
            StunAttributeType::from_raw(0x8027),
            StunAttributeType::CacheTimeout
        );
        assert_eq!(
            StunAttributeType::from_raw(0x802B),
            StunAttributeType::ResponseOrigin
        );
        assert_eq!(
            StunAttributeType::from_raw(0x802C),
            StunAttributeType::OtherAddress
        );
        assert_ne!(
            StunAttributeType::from_raw(0x802D),
            StunAttributeType::OtherAddress
        );
    }

    #[test]
    fn test_rfc5780_codepoints_are_wire_correct() {
        assert_eq!(StunAttributeType::ChangeRequest as u16, 0x0003);
        assert_eq!(StunAttributeType::Padding as u16, 0x0026);
        assert_eq!(StunAttributeType::ResponsePort as u16, 0x0027);
        assert_eq!(StunAttributeType::CacheTimeout as u16, 0x8027);
        assert_eq!(StunAttributeType::ResponseOrigin as u16, 0x802B);
        assert_eq!(StunAttributeType::OtherAddress as u16, 0x802C);
    }
}
