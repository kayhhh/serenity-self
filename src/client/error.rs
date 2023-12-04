use std::error::Error as StdError;
use std::fmt;

/// An error returned from the [`Client`].
///
/// This is always wrapped within the library's generic [`Error::Client`] variant.
///
/// [`Client`]: super::Client
/// [`Error::Client`]: crate::Error::Client
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
#[non_exhaustive]
pub enum Error {
    Shutdown,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Shutdown => f.write_str("The client shutdown"),
        }
    }
}

impl StdError for Error {}
