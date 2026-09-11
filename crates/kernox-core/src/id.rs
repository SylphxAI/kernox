use std::{fmt, str::FromStr};

use crate::IdentifierError;

/// Maximum encoded length of a plugin or capability identifier.
pub const MAX_IDENTIFIER_LENGTH: usize = 255;

/// Maximum encoded length of one identifier segment.
pub const MAX_IDENTIFIER_SEGMENT_LENGTH: usize = 63;

fn validate_identifier(value: &str) -> Result<(), IdentifierError> {
    if value.is_empty() {
        return Err(IdentifierError::Empty);
    }
    if value.len() > MAX_IDENTIFIER_LENGTH {
        return Err(IdentifierError::TooLong {
            actual: value.len(),
            maximum: MAX_IDENTIFIER_LENGTH,
        });
    }
    if !value.is_ascii() {
        return Err(IdentifierError::NonAscii);
    }

    let mut offset = 0;
    for (segment_index, segment) in value.split('.').enumerate() {
        if segment.is_empty() {
            return Err(IdentifierError::EmptySegment { segment: segment_index });
        }
        if segment.len() > MAX_IDENTIFIER_SEGMENT_LENGTH {
            return Err(IdentifierError::SegmentTooLong {
                segment: segment_index,
                actual: segment.len(),
                maximum: MAX_IDENTIFIER_SEGMENT_LENGTH,
            });
        }

        let bytes = segment.as_bytes();
        if !bytes[0].is_ascii_lowercase() {
            return Err(IdentifierError::InvalidSegmentStart {
                segment: segment_index,
                byte_index: offset,
            });
        }

        for (index, byte) in bytes.iter().copied().enumerate().skip(1) {
            if !(byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-') {
                return Err(IdentifierError::InvalidCharacter {
                    byte_index: offset + index,
                    character: char::from(byte),
                });
            }
        }

        if bytes.last() == Some(&b'-') {
            return Err(IdentifierError::InvalidSegmentEnd {
                segment: segment_index,
                byte_index: offset + segment.len() - 1,
            });
        }

        offset += segment.len() + 1;
    }

    Ok(())
}

macro_rules! qualified_id {
    ($name:ident, $doc:literal) => {
        #[doc = $doc]
        #[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name(String);

        impl $name {
            /// Parses and validates a stable identifier.
            ///
            /// # Errors
            ///
            /// Returns [`IdentifierError`] when the value is empty, oversized,
            /// non-ASCII, or contains a non-canonical segment.
            pub fn new(value: impl Into<String>) -> Result<Self, IdentifierError> {
                let value = value.into();
                validate_identifier(&value)?;
                Ok(Self(value))
            }

            /// Returns the canonical string representation.
            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }

            /// Consumes the identifier and returns its canonical representation.
            #[must_use]
            pub fn into_string(self) -> String {
                self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(&self.0)
            }
        }

        impl FromStr for $name {
            type Err = IdentifierError;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                Self::new(value)
            }
        }

        impl TryFrom<String> for $name {
            type Error = IdentifierError;

            fn try_from(value: String) -> Result<Self, Self::Error> {
                Self::new(value)
            }
        }

        impl From<$name> for String {
            fn from(value: $name) -> Self {
                value.into_string()
            }
        }

        #[cfg(feature = "serde")]
        impl serde::Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: serde::Serializer,
            {
                serializer.serialize_str(self.as_str())
            }
        }

        #[cfg(feature = "serde")]
        impl<'de> serde::Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                let value = String::deserialize(deserializer)?;
                Self::new(value).map_err(serde::de::Error::custom)
            }
        }
    };
}

qualified_id!(PluginId, "Stable identity of a plugin, such as `dev.kernox.host.tokio`.");
qualified_id!(CapabilityId, "Stable identity of a capability, such as `dev.kernox.clock`.");

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn accepts_canonical_identifiers() {
        let id = PluginId::new("dev.kernox.host-tokio").unwrap();
        assert_eq!(id.as_str(), "dev.kernox.host-tokio");
    }

    #[test]
    fn rejects_non_canonical_identifiers() {
        let invalid = [
            "",
            ".dev.kernox",
            "dev..kernox",
            "dev.kernox.",
            "Dev.kernox",
            "dev.kernox_thing",
            "dev.kernox-",
            "dev.kérnox",
        ];

        for value in invalid {
            assert!(PluginId::new(value).is_err(), "identifier should fail: {value}");
        }
    }

    #[cfg(feature = "serde")]
    #[test]
    fn deserialization_revalidates_identifiers() {
        assert_eq!(PluginId::new("Invalid").unwrap_err().tag(), "identifier.invalid-segment-start");
        let parsed: Result<PluginId, _> = serde_json::from_str("\"Invalid\"");
        assert!(parsed.is_err());
        let accepted: PluginId = serde_json::from_str("\"dev.kernox.host-tokio\"").unwrap();
        assert_eq!(accepted.as_str(), "dev.kernox.host-tokio");
    }

    #[test]
    fn enforces_the_total_length_boundary() {
        // Four maximum-length segments plus separators reach exactly 255 bytes.
        let at_boundary =
            [MAX_IDENTIFIER_SEGMENT_LENGTH; 4].map(|length| "a".repeat(length)).join(".");
        assert_eq!(at_boundary.len(), MAX_IDENTIFIER_LENGTH);
        assert_eq!(PluginId::new(at_boundary).unwrap().as_str().len(), MAX_IDENTIFIER_LENGTH);

        // Five shorter segments reach 256 bytes, one byte past the total bound.
        let over_boundary = [63_usize, 63, 63, 62, 1].map(|length| "a".repeat(length)).join(".");
        assert_eq!(over_boundary.len(), MAX_IDENTIFIER_LENGTH + 1);
        assert_eq!(
            PluginId::new(over_boundary).unwrap_err(),
            IdentifierError::TooLong {
                actual: MAX_IDENTIFIER_LENGTH + 1,
                maximum: MAX_IDENTIFIER_LENGTH,
            }
        );
    }

    #[test]
    fn enforces_the_segment_length_boundary() {
        let at_boundary = "a".repeat(MAX_IDENTIFIER_SEGMENT_LENGTH);
        assert_eq!(
            PluginId::new(at_boundary).unwrap().as_str().len(),
            MAX_IDENTIFIER_SEGMENT_LENGTH
        );

        assert_eq!(
            PluginId::new("a".repeat(MAX_IDENTIFIER_SEGMENT_LENGTH + 1)).unwrap_err(),
            IdentifierError::SegmentTooLong {
                segment: 0,
                actual: MAX_IDENTIFIER_SEGMENT_LENGTH + 1,
                maximum: MAX_IDENTIFIER_SEGMENT_LENGTH,
            }
        );
    }

    #[test]
    fn reports_absolute_byte_offsets_for_later_segments() {
        assert_eq!(
            PluginId::new("dev.bad@cap").unwrap_err(),
            IdentifierError::InvalidCharacter { byte_index: 7, character: '@' }
        );
        assert_eq!(
            PluginId::new("dev.example-").unwrap_err(),
            IdentifierError::InvalidSegmentEnd { segment: 1, byte_index: 11 }
        );
    }
}
