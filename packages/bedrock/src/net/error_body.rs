use serde::Deserialize;

/// The body Copal, the store and the backend answer an error with: `{error, description}`, plus
/// the backend's own `code` when it names one.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct ErrorBody {
    #[serde(default)]
    pub error: String,
    #[serde(default)]
    pub code: Option<String>,
    #[serde(default)]
    pub description: String,
}

impl ErrorBody {
    /// `body` as an error body, `None` when it is not a JSON object.
    pub fn parse(body: &[u8]) -> Option<Self> {
        serde_json::from_slice::<serde_json::Map<String, serde_json::Value>>(
            body,
        )
        .ok()
        .and_then(|object| {
            serde_json::from_value(serde_json::Value::Object(object)).ok()
        })
    }

    /// The backend's `code` when it names one, otherwise the `error` kind.
    pub fn code(&self) -> &str {
        self.code.as_deref().unwrap_or(&self.error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_code_falls_back_to_the_error_kind() {
        let body = ErrorBody::parse(
            br#"{"error":"forbidden","code":"no_storage","description":"No Core."}"#,
        )
        .unwrap();
        assert_eq!(body.code(), "no_storage");
        assert_eq!(body.description, "No Core.");
        let body = ErrorBody::parse(br#"{"error":"disk_full"}"#).unwrap();
        assert_eq!(body.code(), "disk_full");
        assert_eq!(body.description, "");
        assert_eq!(ErrorBody::parse(b"Bad Gateway"), None);
        assert_eq!(ErrorBody::parse(b"[]"), None);
    }
}
