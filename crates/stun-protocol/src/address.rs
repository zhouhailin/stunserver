//! Network address types for STUN attributes

use crate::error::StunError;
use byteorder::{BigEndian, ByteOrder};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

/// Address family (IPv4 or IPv6)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum AddressFamily {
    /// IPv4 address (family value 0x01)
    IPv4 = 0x01,
    /// IPv6 address (family value 0x02)
    IPv6 = 0x02,
}

impl AddressFamily {
    /// Convert from raw u8 value
    pub fn from_raw(value: u8) -> Result<Self, StunError> {
        match value {
            0x01 => Ok(AddressFamily::IPv4),
            0x02 => Ok(AddressFamily::IPv6),
            _ => Err(StunError::MalformedMessage(format!(
                "Invalid address family: 0x{:02X}",
                value
            ))),
        }
    }
}

/// Mapped network address
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MappedAddress {
    /// Address family
    pub family: AddressFamily,
    /// Port number
    pub port: u16,
    /// IP address
    pub address: IpAddr,
}

impl MappedAddress {
    /// Create a new mapped address
    pub fn new(address: IpAddr, port: u16) -> Self {
        let family = match address {
            IpAddr::V4(_) => AddressFamily::IPv4,
            IpAddr::V6(_) => AddressFamily::IPv6,
        };
        Self {
            family,
            port,
            address,
        }
    }

    /// Encode to byte buffer (for MAPPED-ADDRESS attribute)
    pub fn encode(&self) -> Vec<u8> {
        let mut buffer = Vec::new();

        // Reserved byte (0x00)
        buffer.push(0x00);

        // Address family (1 byte)
        buffer.push(self.family as u8);

        // Port (2 bytes)
        let mut port_bytes = [0u8; 2];
        BigEndian::write_u16(&mut port_bytes, self.port);
        buffer.extend_from_slice(&port_bytes);

        // Address (4 or 16 bytes)
        match self.address {
            IpAddr::V4(ipv4) => {
                buffer.extend_from_slice(&ipv4.octets());
            }
            IpAddr::V6(ipv6) => {
                buffer.extend_from_slice(&ipv6.octets());
            }
        }

        buffer
    }

    /// Decode from byte buffer (for MAPPED-ADDRESS attribute)
    pub fn decode(data: &[u8]) -> Result<Self, StunError> {
        if data.len() < 4 {
            return Err(StunError::MalformedMessage(
                "MappedAddress data too short".to_string(),
            ));
        }

        // Skip reserved byte
        let family_raw = data[1];
        let family = AddressFamily::from_raw(family_raw)?;

        let port = BigEndian::read_u16(&data[2..4]);

        let address = match family {
            AddressFamily::IPv4 => {
                if data.len() < 8 {
                    return Err(StunError::MalformedMessage(
                        "IPv4 address truncated".to_string(),
                    ));
                }
                let mut octets = [0u8; 4];
                octets.copy_from_slice(&data[4..8]);
                IpAddr::V4(Ipv4Addr::from(octets))
            }
            AddressFamily::IPv6 => {
                if data.len() < 20 {
                    return Err(StunError::MalformedMessage(
                        "IPv6 address truncated".to_string(),
                    ));
                }
                let mut octets = [0u8; 16];
                octets.copy_from_slice(&data[4..20]);
                IpAddr::V6(Ipv6Addr::from(octets))
            }
        };

        Ok(Self {
            family,
            port,
            address,
        })
    }

    /// Encode as XOR-MAPPED-ADDRESS (XOR with magic cookie and transaction ID)
    pub fn encode_xor(&self, magic_cookie: u32, transaction_id: &[u8; 12]) -> Vec<u8> {
        let mut buffer = Vec::new();

        // Reserved byte (0x00)
        buffer.push(0x00);

        // Address family (1 byte)
        buffer.push(self.family as u8);

        // XOR port with top 16 bits of magic cookie
        let xor_port = self.port ^ ((magic_cookie >> 16) as u16);
        let mut port_bytes = [0u8; 2];
        BigEndian::write_u16(&mut port_bytes, xor_port);
        buffer.extend_from_slice(&port_bytes);

        // XOR address
        match self.address {
            IpAddr::V4(ipv4) => {
                let xor_ip = u32::from(ipv4) ^ magic_cookie;
                buffer.extend_from_slice(&xor_ip.to_be_bytes());
            }
            IpAddr::V6(ipv6) => {
                let mut xor_bytes = [0u8; 16];
                let ip_bytes = ipv6.octets();

                // XOR with magic cookie (4 bytes) + transaction ID (12 bytes)
                let mut cookie_bytes = [0u8; 4];
                BigEndian::write_u32(&mut cookie_bytes, magic_cookie);

                for i in 0..4 {
                    xor_bytes[i] = ip_bytes[i] ^ cookie_bytes[i];
                }
                for i in 0..12 {
                    xor_bytes[4 + i] = ip_bytes[4 + i] ^ transaction_id[i];
                }

                buffer.extend_from_slice(&xor_bytes);
            }
        }

        buffer
    }

    /// Decode from XOR-MAPPED-ADDRESS
    pub fn decode_xor(
        data: &[u8],
        magic_cookie: u32,
        transaction_id: &[u8; 12],
    ) -> Result<Self, StunError> {
        if data.len() < 4 {
            return Err(StunError::MalformedMessage(
                "XorMappedAddress data too short".to_string(),
            ));
        }

        // Skip reserved byte
        let family_raw = data[1];
        let family = AddressFamily::from_raw(family_raw)?;

        // XOR port
        let xor_port = BigEndian::read_u16(&data[2..4]);
        let port = xor_port ^ ((magic_cookie >> 16) as u16);

        let address = match family {
            AddressFamily::IPv4 => {
                if data.len() < 8 {
                    return Err(StunError::MalformedMessage(
                        "IPv4 address truncated".to_string(),
                    ));
                }
                let mut xor_bytes = [0u8; 4];
                xor_bytes.copy_from_slice(&data[4..8]);
                let ip_int = u32::from_be_bytes(xor_bytes) ^ magic_cookie;
                IpAddr::V4(Ipv4Addr::from(ip_int))
            }
            AddressFamily::IPv6 => {
                if data.len() < 20 {
                    return Err(StunError::MalformedMessage(
                        "IPv6 address truncated".to_string(),
                    ));
                }
                let mut xor_bytes = [0u8; 16];
                xor_bytes.copy_from_slice(&data[4..20]);

                let mut ip_bytes = [0u8; 16];
                let mut cookie_bytes = [0u8; 4];
                BigEndian::write_u32(&mut cookie_bytes, magic_cookie);

                for i in 0..4 {
                    ip_bytes[i] = xor_bytes[i] ^ cookie_bytes[i];
                }
                for i in 0..12 {
                    ip_bytes[4 + i] = xor_bytes[4 + i] ^ transaction_id[i];
                }

                IpAddr::V6(Ipv6Addr::from(ip_bytes))
            }
        };

        Ok(Self {
            family,
            port,
            address,
        })
    }
}
