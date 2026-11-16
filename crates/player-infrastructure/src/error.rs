use std::fmt;

/// Technical diagnostics stay separate from translated application messages.
#[derive(Debug)]
pub struct InfraError {
    pub code: &'static str,
    pub detail: String,
}

impl InfraError {
    pub fn new(code: &'static str, detail: impl Into<String>) -> Self {
        Self {
            code,
            detail: detail.into(),
        }
    }
}

impl fmt::Display for InfraError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.code, self.detail)
    }
}

impl std::error::Error for InfraError {}

impl From<std::io::Error> for InfraError {
    fn from(error: std::io::Error) -> Self {
        Self::new("io", error.to_string())
    }
}

impl From<rusqlite::Error> for InfraError {
    fn from(error: rusqlite::Error) -> Self {
        Self::new("database", error.to_string())
    }
}
