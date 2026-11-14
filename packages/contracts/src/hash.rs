use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer, de};

/// A SHA-256, written as 64 lowercase hex characters and read back only in that spelling, so the
/// text is safe in a path. Sorting the text sorts the bytes.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Hash(#[cfg_attr(feature = "ts", ts(type = "string"))] pub [u8; 32]);

impl Hash {
    pub fn parse(text: &str) -> Option<Self> {
        fn nibble(digit: u8) -> Option<u8> {
            match digit {
                b'0'..=b'9' => Some(digit - b'0'),
                b'a'..=b'f' => Some(digit - b'a' + 10),
                _ => None,
            }
        }

        let text = text.as_bytes();
        if text.len() != 64 {
            return None;
        }
        let mut bytes = [0; 32];
        for (byte, pair) in bytes.iter_mut().zip(text.chunks_exact(2)) {
            *byte = (nibble(pair[0])? << 4) | nibble(pair[1])?;
        }
        Some(Self(bytes))
    }
}

impl fmt::Display for Hash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.iter().try_for_each(|byte| write!(f, "{byte:02x}"))
    }
}

impl fmt::Debug for Hash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

impl Serialize for Hash {
    fn serialize<S: Serializer>(
        &self,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Hash {
    fn deserialize<D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        Self::parse(&text).ok_or_else(|| {
            de::Error::custom("a hash is 64 lowercase hex characters")
        })
    }
}

#[cfg(test)]
mod tests {
    use super::Hash;

    #[test]
    fn only_lowercase_hex_of_the_right_length_is_a_hash() {
        let text = "0f".repeat(32);
        let hash = Hash::parse(&text).unwrap();
        assert_eq!(hash.0, [0x0f; 32]);
        assert_eq!(hash.to_string(), text);
        for bad in [
            "A".repeat(64),
            "a".repeat(63),
            "a".repeat(65),
            "../".repeat(21) + "a",
            "g".repeat(64),
        ] {
            assert!(Hash::parse(&bad).is_none(), "{bad}");
        }
        assert!(serde_json::from_str::<Hash>("\"../../etc/passwd\"").is_err());
        assert_eq!(
            serde_json::to_string(&hash).unwrap(),
            format!("\"{text}\"")
        );
    }
}
