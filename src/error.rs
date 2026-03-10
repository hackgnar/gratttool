use std::fmt;

/// ATT error codes matching BlueZ's att_ecode2str()
#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub struct AttError(pub u8);

impl AttError {
    #[allow(dead_code)]
    pub fn as_str(&self) -> &'static str {
        match self.0 {
            0x01 => "Invalid handle",
            0x02 => "Attribute can't be read",
            0x03 => "Attribute can't be written",
            0x04 => "Attribute PDU was invalid",
            0x05 => "Attribute requires authentication before read/write",
            0x06 => "Server doesn't support the request received",
            0x07 => "Offset past the end of the attribute",
            0x08 => "Attribute requires authorization before read/write",
            0x09 => "Too many prepare writes have been queued",
            0x0A => "No attribute found within the given range",
            0x0B => "Attribute can't be read/written using Read Blob Req",
            0x0C => "Encryption Key Size is insufficient",
            0x0D => "Attribute value length is invalid",
            0x0E => "Request attribute has encountered an unlikely error",
            0x0F => "Encryption required before read/write",
            0x10 => "Attribute type is not a supported grouping attribute",
            0x11 => "Insufficient Resources to complete the request",
            0x80 => "Internal application error: I/O",
            0x81 => "A timeout occurred",
            0x82 => "The operation was aborted",
            _ => "Unexpected error code",
        }
    }
}

impl fmt::Display for AttError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Application-level error type
#[derive(Debug)]
#[allow(dead_code)]
pub enum GrattError {
    Adapter(String),
    Connection(String),
    Gatt(String),
    InvalidHandle(String),
    InvalidValue(String),
    InvalidUuid(String),
    NotConnected,
    Bluer(bluer::Error),
    Io(std::io::Error),
}

impl fmt::Display for GrattError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GrattError::Adapter(msg) => write!(f, "{}", msg),
            GrattError::Connection(msg) => write!(f, "{}", msg),
            GrattError::Gatt(msg) => write!(f, "{}", msg),
            GrattError::InvalidHandle(msg) => write!(f, "{}", msg),
            GrattError::InvalidValue(msg) => write!(f, "{}", msg),
            GrattError::InvalidUuid(msg) => write!(f, "{}", msg),
            GrattError::NotConnected => write!(f, "Disconnected"),
            GrattError::Bluer(e) => write!(f, "{}", e),
            GrattError::Io(e) => write!(f, "{}", e),
        }
    }
}

impl std::error::Error for GrattError {}

impl From<bluer::Error> for GrattError {
    fn from(e: bluer::Error) -> Self {
        GrattError::Bluer(e)
    }
}

impl From<std::io::Error> for GrattError {
    fn from(e: std::io::Error) -> Self {
        GrattError::Io(e)
    }
}
