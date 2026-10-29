//! Network address parsing and DNS resolution for STUN client.
//!
//! Supports IP addresses, domain names, and optional port specification.
//! Defaults to port 3478 when no port is provided.

use std::fmt;
use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr, ToSocketAddrs};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AddressError {
    #[error("Invalid address format: {0}")]
    InvalidFormat(String),

    #[error("DNS resolution failed for '{0}': {1}")]
    DnsResolutionFailed(String, String),

    #[error("No addresses found for '{0}'")]
    NoAddressesFound(String),
}

#[derive(Debug, Clone)]
pub enum Host {
    IPv4(Ipv4Addr),
    IPv6(Ipv6Addr),
    Domain(String),
}

impl fmt::Display for Host {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Host::IPv4(addr) => write!(f, "{}", addr),
            Host::IPv6(addr) => write!(f, "{}", addr),
            Host::Domain(domain) => write!(f, "{}", domain),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AddressFamily {
    #[default]
    Any,
    IPv4Only,
    IPv6Only,
}

#[derive(Debug, Clone)]
pub struct NetworkAddress {
    pub original: String,
    pub host: Host,
    pub port: u16,
    pub resolved_addrs: Vec<SocketAddr>,
    pub family: AddressFamily,
}

impl NetworkAddress {
    pub fn parse(addr_str: &str, default_port: u16) -> Result<Self, AddressError> {
        Self::parse_with_family(addr_str, default_port, AddressFamily::Any)
    }

    pub fn parse_with_family(
        addr_str: &str,
        default_port: u16,
        family: AddressFamily,
    ) -> Result<Self, AddressError> {
        let addr_str = addr_str.trim();

        if addr_str.is_empty() {
            return Err(AddressError::InvalidFormat("Empty address".to_string()));
        }

        let (host_str, port) = Self::parse_host_port(addr_str, default_port)?;
        let host = Self::parse_host(&host_str)?;

        Ok(Self {
            original: addr_str.to_string(),
            host,
            port,
            resolved_addrs: Vec::new(),
            family,
        })
    }

    fn parse_host_port(addr_str: &str, default_port: u16) -> Result<(String, u16), AddressError> {
        if addr_str.starts_with('[') {
            let end_bracket = addr_str.find(']').ok_or_else(|| {
                AddressError::InvalidFormat("Missing closing bracket for IPv6".to_string())
            })?;

            let host = addr_str[1..end_bracket].to_string();
            let remaining = &addr_str[end_bracket + 1..];

            let port = if remaining.is_empty() {
                default_port
            } else if let Some(port_str) = remaining.strip_prefix(':') {
                port_str.parse::<u16>().map_err(|_| {
                    AddressError::InvalidFormat(format!("Invalid port: {}", port_str))
                })?
            } else {
                return Err(AddressError::InvalidFormat(format!(
                    "Invalid format after IPv6: {}",
                    remaining
                )));
            };

            Ok((host, port))
        } else if addr_str.contains(':') {
            let parts: Vec<&str> = addr_str.rsplitn(2, ':').collect();
            if parts.len() == 2 {
                let port_str = parts[0];
                let host = parts[1].to_string();

                if let Ok(port) = port_str.parse::<u16>() {
                    // Validate that host doesn't contain colons (invalid for IPv4/domain)
                    if host.contains(':') {
                        return Err(AddressError::InvalidFormat(format!(
                            "'{}'. For IPv6, use brackets: [IPv6]:port. Example: [::1]:3478",
                            addr_str
                        )));
                    }
                    Ok((host, port))
                } else {
                    Ok((addr_str.to_string(), default_port))
                }
            } else {
                Ok((addr_str.to_string(), default_port))
            }
        } else {
            Ok((addr_str.to_string(), default_port))
        }
    }

    fn parse_host(host_str: &str) -> Result<Host, AddressError> {
        if let Ok(ipv4) = host_str.parse::<Ipv4Addr>() {
            Ok(Host::IPv4(ipv4))
        } else if let Ok(ipv6) = host_str.parse::<Ipv6Addr>() {
            Ok(Host::IPv6(ipv6))
        } else {
            if host_str.is_empty() {
                return Err(AddressError::InvalidFormat("Empty hostname".to_string()));
            }
            Ok(Host::Domain(host_str.to_string()))
        }
    }

    pub fn resolve(&mut self) -> Result<(), AddressError> {
        let addr_string = match &self.host {
            Host::IPv4(ip) => format!("{}:{}", ip, self.port),
            Host::IPv6(ip) => format!("[{}]:{}", ip, self.port),
            Host::Domain(domain) => format!("{}:{}", domain, self.port),
        };

        let mut addrs: Vec<SocketAddr> = addr_string
            .to_socket_addrs()
            .map_err(|e| AddressError::DnsResolutionFailed(addr_string.clone(), e.to_string()))?
            .collect();

        if addrs.is_empty() {
            return Err(AddressError::NoAddressesFound(addr_string));
        }

        // Filter by address family if specified.
        match self.family {
            AddressFamily::IPv4Only => {
                addrs.retain(|a| a.is_ipv4());
            }
            AddressFamily::IPv6Only => {
                addrs.retain(|a| a.is_ipv6());
            }
            AddressFamily::Any => {}
        }

        if addrs.is_empty() {
            let family_str = match self.family {
                AddressFamily::IPv4Only => "IPv4",
                AddressFamily::IPv6Only => "IPv6",
                AddressFamily::Any => "any",
            };
            return Err(AddressError::NoAddressesFound(format!(
                "{} ({})",
                addr_string, family_str
            )));
        }

        addrs.sort_by_key(|a| match a {
            SocketAddr::V4(_) => 0,
            SocketAddr::V6(_) => 1,
        });

        self.resolved_addrs = addrs;
        Ok(())
    }

    pub fn primary_address(&self) -> Option<SocketAddr> {
        self.resolved_addrs.first().copied()
    }
}

impl fmt::Display for NetworkAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.host {
            Host::IPv4(ip) => write!(f, "{}:{}", ip, self.port),
            Host::IPv6(ip) => write!(f, "[{}]:{}", ip, self.port),
            Host::Domain(domain) => write!(f, "{}:{}", domain, self.port),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_ipv4_only() {
        let addr = NetworkAddress::parse("127.0.0.1", 3478).unwrap();
        assert!(matches!(addr.host, Host::IPv4(_)));
        assert_eq!(addr.port, 3478);
    }

    #[test]
    fn test_parse_ipv4_with_port() {
        let addr = NetworkAddress::parse("127.0.0.1:3479", 3478).unwrap();
        assert!(matches!(addr.host, Host::IPv4(_)));
        assert_eq!(addr.port, 3479);
    }

    #[test]
    fn test_parse_ipv6_with_brackets() {
        let addr = NetworkAddress::parse("[::1]", 3478).unwrap();
        assert!(matches!(addr.host, Host::IPv6(_)));
        assert_eq!(addr.port, 3478);
    }

    #[test]
    fn test_parse_ipv6_with_port() {
        let addr = NetworkAddress::parse("[::1]:3479", 3478).unwrap();
        assert!(matches!(addr.host, Host::IPv6(_)));
        assert_eq!(addr.port, 3479);
    }

    #[test]
    fn test_parse_domain_only() {
        let addr = NetworkAddress::parse("example.com", 3478).unwrap();
        assert!(matches!(addr.host, Host::Domain(_)));
        assert_eq!(addr.port, 3478);
    }

    #[test]
    fn test_parse_domain_with_port() {
        let addr = NetworkAddress::parse("example.com:3479", 3478).unwrap();
        assert!(matches!(addr.host, Host::Domain(_)));
        assert_eq!(addr.port, 3479);
    }

    #[test]
    fn test_resolve_localhost() {
        let mut addr = NetworkAddress::parse("127.0.0.1:3478", 3478).unwrap();
        addr.resolve().unwrap();
        assert!(!addr.resolved_addrs.is_empty());
    }

    #[test]
    fn test_invalid_empty_address() {
        let result = NetworkAddress::parse("", 3478);
        assert!(result.is_err());
    }

    #[test]
    fn test_address_family_default() {
        assert_eq!(AddressFamily::default(), AddressFamily::Any);
    }

    #[test]
    fn test_parse_with_family_ipv4_only() {
        let addr =
            NetworkAddress::parse_with_family("127.0.0.1", 3478, AddressFamily::IPv4Only).unwrap();
        assert_eq!(addr.family, AddressFamily::IPv4Only);
    }

    #[test]
    fn test_parse_with_family_ipv6_only() {
        let addr =
            NetworkAddress::parse_with_family("[::1]", 3478, AddressFamily::IPv6Only).unwrap();
        assert_eq!(addr.family, AddressFamily::IPv6Only);
    }

    #[test]
    fn test_resolve_filters_ipv4() {
        let mut addr =
            NetworkAddress::parse_with_family("localhost", 3478, AddressFamily::IPv4Only).unwrap();
        addr.resolve().unwrap();
        assert!(!addr.resolved_addrs.is_empty());
        assert!(addr.resolved_addrs.iter().all(|a| a.is_ipv4()));
    }

    #[test]
    fn test_resolve_filters_ipv6() {
        let mut addr =
            NetworkAddress::parse_with_family("localhost", 3478, AddressFamily::IPv6Only).unwrap();
        addr.resolve().unwrap();
        // localhost may not have IPv6 resolution, so we just check the filter was applied
        assert!(addr.resolved_addrs.iter().all(|a| a.is_ipv6()));
    }
}
