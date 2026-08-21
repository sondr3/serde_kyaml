use std::fmt;

/// Errors that can occur while serializing to or deserializing from KYAML.
#[derive(Debug)]
pub enum Error {
    /// The value could not be converted to a JSON-compatible tree
    /// (e.g. a map with a non-string-convertible key).
    Json(serde_json::Error),
    /// The input could not be parsed as YAML, or did not match the target type.
    Yaml(yaml_serde::Error),
    /// Writing the output failed.
    Io(std::io::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Json(e) => e.fmt(f),
            Error::Yaml(e) => e.fmt(f),
            Error::Io(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Json(e) => Some(e),
            Error::Yaml(e) => Some(e),
            Error::Io(e) => Some(e),
        }
    }
}

impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Error::Json(e)
    }
}

impl From<yaml_serde::Error> for Error {
    fn from(e: yaml_serde::Error) -> Self {
        Error::Yaml(e)
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e)
    }
}
