//! STUN protocol error types

use thiserror::Error;

/// STUN protocol errors
#[derive(Error, Debug)]
pub enum StunError {
    /// Invalid message type
    #[error("Invalid message type: {0}")]
    InvalidMessageType(u16),

    /// Missing required attribute
    #[error("Missing required attribute: {0}")]
    MissingAttribute(String),

    /// Malformed message
    #[error("Malformed message: {0}")]
    MalformedMessage(String),
}
