//! The error type of every fallible call.

use std::fmt;

/// `Result<T, probnode::Error>`.
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Why a call failed.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// The API answered with an error status. `detail` is the API's own sentence (for example
    /// `"missing or invalid api key"`), or the validation errors as JSON for a 422.
    Api {
        /// The HTTP status code.
        status: u16,
        /// What the API said.
        detail: String,
    },
    /// A call that needs an API key was made without one. Nothing was sent.
    NoApiKey,
    /// A downloaded file's sha256 is not the one the API listed. The partial file was removed.
    Checksum {
        /// The object's path in the store.
        object_path: String,
        /// The sha256 the API listed.
        expected: String,
        /// The sha256 of the bytes received.
        actual: String,
    },
    /// The request could not be made or its answer not read (DNS, TLS, connection, timeout).
    Http(reqwest::Error),
    /// Reading or writing a local file failed.
    Io(std::io::Error),
    /// A response or the saved config was not the JSON expected.
    Json(serde_json::Error),
    /// An argument or a response that the client refuses, such as an object path that would
    /// leave the destination directory.
    Invalid(String),
    /// Opening a Probe file failed (feature `probe`).
    #[cfg(feature = "probe")]
    Probe(probe_format::Error),
}

impl Error {
    /// The HTTP status of an [`Error::Api`]; 401 for [`Error::NoApiKey`]; `None` otherwise.
    pub fn status(&self) -> Option<u16> {
        match self {
            Error::Api { status, .. } => Some(*status),
            Error::NoApiKey => Some(401),
            Error::Http(e) => e.status().map(|s| s.as_u16()),
            _ => None,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Api { status, detail } => write!(f, "{status}: {detail}"),
            Error::NoApiKey => {
                f.write_str("no API key: pass one, set PROBNODE_API_KEY, or run `probnode login`")
            }
            Error::Checksum {
                object_path,
                expected,
                actual,
            } => write!(
                f,
                "sha256 mismatch for {object_path}: expected {expected}, got {actual}"
            ),
            Error::Http(e) => write!(f, "http: {e}"),
            Error::Io(e) => write!(f, "io: {e}"),
            Error::Json(e) => write!(f, "json: {e}"),
            Error::Invalid(m) => f.write_str(m),
            #[cfg(feature = "probe")]
            Error::Probe(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Http(e) => Some(e),
            Error::Io(e) => Some(e),
            Error::Json(e) => Some(e),
            #[cfg(feature = "probe")]
            Error::Probe(e) => Some(e),
            _ => None,
        }
    }
}

impl From<reqwest::Error> for Error {
    fn from(e: reqwest::Error) -> Self {
        Error::Http(e)
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e)
    }
}

impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Error::Json(e)
    }
}

#[cfg(feature = "probe")]
impl From<probe_format::Error> for Error {
    fn from(e: probe_format::Error) -> Self {
        Error::Probe(e)
    }
}
