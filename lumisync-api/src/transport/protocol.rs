use super::error::TransportError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Protocol {
    /// Postcard binary protocol (default)
    Postcard = 1,
    /// JSON text protocol  
    Json = 2,
}

impl Default for Protocol {
    fn default() -> Self {
        Self::Postcard
    }
}

impl Protocol {
    /// Creates protocol from byte value
    pub fn from_u8(value: u8) -> Result<Self, TransportError> {
        match value {
            1 => Ok(Self::Postcard),
            2 => Ok(Self::Json),
            other => Err(TransportError::UnknownProtocol(other)),
        }
    }

    /// Returns protocol name
    pub fn name(&self) -> &'static str {
        match self {
            Self::Postcard => "postcard",
            Self::Json => "json",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_protocol_from_u8() {
        assert_eq!(Protocol::from_u8(1).unwrap(), Protocol::Postcard);
        assert_eq!(Protocol::from_u8(2).unwrap(), Protocol::Json);
        assert!(Protocol::from_u8(99).is_err());
    }

    #[test]
    fn test_protocol_name() {
        assert_eq!(Protocol::Postcard.name(), "postcard");
        assert_eq!(Protocol::Json.name(), "json");
    }

    #[test]
    fn test_protocol_default() {
        assert_eq!(Protocol::default(), Protocol::Postcard);
    }
}
