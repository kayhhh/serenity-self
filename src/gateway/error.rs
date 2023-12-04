use std::error::Error as StdError;
use std::fmt;

use tokio_tungstenite::tungstenite::protocol::CloseFrame;

/// An error that occurred while attempting to deal with the gateway.
///
/// Note that - from a user standpoint - there should be no situation in which you manually handle
/// these.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub enum Error {
    /// There was an error building a URL.
    BuildingUrl,
    /// The connection closed, potentially uncleanly.
    Closed(Option<CloseFrame<'static>>),
    /// Expected a Hello during a handshake
    ExpectedHello,
    /// When there was an error sending a heartbeat.
    HeartbeatFailed,
    /// When invalid authentication (a bad token) was sent in the IDENTIFY.
    InvalidAuthentication,
    /// Expected a Ready or an InvalidateSession
    InvalidHandshake,
    /// When no authentication was sent in the IDENTIFY.
    NoAuthentication,
    /// When a session Id was expected (for resuming), but was not present.
    NoSessionId,
    /// Failed to reconnect after a number of attempts.
    ReconnectFailure,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BuildingUrl => f.write_str("Error building url"),
            Self::Closed(_) => f.write_str("Connection closed"),
            Self::ExpectedHello => f.write_str("Expected a Hello"),
            Self::HeartbeatFailed => f.write_str("Failed sending a heartbeat"),
            Self::InvalidAuthentication => f.write_str("Sent invalid authentication"),
            Self::InvalidHandshake => f.write_str("Expected a valid Handshake"),
            Self::NoAuthentication => f.write_str("Sent no authentication"),
            Self::NoSessionId => f.write_str("No Session Id present when required"),
            Self::ReconnectFailure => f.write_str("Failed to Reconnect"),
        }
    }
}

impl StdError for Error {}
