//! STUN Protocol Library
//!
//! This library provides STUN (Session Traversal Utilities for NAT) protocol
//! implementation according to RFC 5389.

pub mod address;
pub mod attributes;
pub mod error;
pub mod message;
pub mod summary;

pub use address::{AddressFamily, MappedAddress};
pub use attributes::{StunAttribute, StunAttributeType};
pub use error::StunError;
pub use message::{StunMessage, StunMessageType};
pub use summary::{describe_attribute, describe_message};
