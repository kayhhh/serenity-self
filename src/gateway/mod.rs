//! The gateway module contains the pieces - responsible for maintaining a
//! WebSocket connection with Discord.
//!
//! An interface for the lower-level receiver and sender. It provides what can otherwise
//! be thought of as "sugar methods".

mod error;
mod ws;

use std::fmt;

#[cfg(feature = "http")]
use reqwest::IntoUrl;
use reqwest::Url;

pub use self::error::Error as GatewayError;
pub use self::ws::WsClient;
#[cfg(feature = "http")]
use crate::internal::prelude::*;
use crate::model::gateway::{Activity, ActivityType};
use crate::model::id::UserId;
use crate::model::user::OnlineStatus;

/// Presence data of the current user.
#[derive(Clone, Debug, Default)]
pub struct PresenceData {
    /// The current activity, if present
    pub activity: Option<ActivityData>,
    /// The current online status
    pub status: OnlineStatus,
}

/// Activity data of the current user.
#[derive(Clone, Debug, Serialize)]
pub struct ActivityData {
    /// The name of the activity
    pub name: String,
    /// The type of the activity
    #[serde(rename = "type")]
    pub kind: ActivityType,
    /// The state of the activity, if the type is [`ActivityType::Custom`]
    pub state: Option<String>,
    /// The url of the activity, if the type is [`ActivityType::Streaming`]
    pub url: Option<Url>,
}

impl ActivityData {
    /// Creates an activity that appears as `Playing <name>`.
    #[must_use]
    pub fn playing(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            kind: ActivityType::Playing,
            state: None,
            url: None,
        }
    }

    /// Creates an activity that appears as `Streaming <name>`.
    ///
    /// # Errors
    ///
    /// Returns an error if the URL parsing fails.
    #[cfg(feature = "http")]
    pub fn streaming(name: impl Into<String>, url: impl IntoUrl) -> Result<Self> {
        Ok(Self {
            name: name.into(),
            kind: ActivityType::Streaming,
            state: None,
            url: Some(url.into_url()?),
        })
    }

    /// Creates an activity that appears as `Listening to <name>`.
    #[must_use]
    pub fn listening(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            kind: ActivityType::Listening,
            state: None,
            url: None,
        }
    }

    /// Creates an activity that appears as `Watching <name>`.
    #[must_use]
    pub fn watching(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            kind: ActivityType::Watching,
            state: None,
            url: None,
        }
    }

    /// Creates an activity that appears as `Competing in <name>`.
    #[must_use]
    pub fn competing(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            kind: ActivityType::Competing,
            state: None,
            url: None,
        }
    }

    /// Creates an activity that appears as `<state>`.
    #[must_use]
    pub fn custom(state: impl Into<String>) -> Self {
        Self {
            // discord seems to require a name for custom activities
            // even though it's not displayed
            name: "~".to_string(),
            kind: ActivityType::Custom,
            state: Some(state.into()),
            url: None,
        }
    }
}

impl From<Activity> for ActivityData {
    fn from(activity: Activity) -> Self {
        Self {
            name: activity.name,
            kind: activity.kind,
            state: activity.state,
            url: activity.url,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[non_exhaustive]
pub enum ConnectionStage {
    /// Indicator that it is normally connected and is not in, e.g., a resume phase.
    Connected,
    /// Indicator that it is connecting and is in, e.g., a resume phase.
    Connecting,
    /// Indicator that it is fully disconnected and is not in a reconnecting phase.
    Disconnected,
    /// Indicator that it is currently initiating a handshake.
    Handshake,
    /// Indicator that it has sent an IDENTIFY packet and is awaiting a READY packet.
    Identifying,
    /// Indicator that it has sent a RESUME packet and is awaiting a RESUMED packet.
    Resuming,
}

impl ConnectionStage {
    /// Whether the stage is a form of connecting.
    ///
    /// This will return `true` on:
    /// - [`Connecting`][`ConnectionStage::Connecting`]
    /// - [`Handshake`][`ConnectionStage::Handshake`]
    /// - [`Identifying`][`ConnectionStage::Identifying`]
    /// - [`Resuming`][`ConnectionStage::Resuming`]
    ///
    /// All other variants will return `false`.
    ///
    /// # Examples
    ///
    /// Assert that [`ConnectionStage::Identifying`] is a connecting stage:
    ///
    /// ```rust
    /// use serenity::gateway::ConnectionStage;
    ///
    /// assert!(ConnectionStage::Identifying.is_connecting());
    /// ```
    ///
    /// Assert that [`ConnectionStage::Connected`] is _not_ a connecting stage:
    ///
    /// ```rust
    /// use serenity::gateway::ConnectionStage;
    ///
    /// assert!(!ConnectionStage::Connected.is_connecting());
    /// ```
    #[must_use]
    pub fn is_connecting(self) -> bool {
        use self::ConnectionStage::{Connecting, Handshake, Identifying, Resuming};
        matches!(self, Connecting | Handshake | Identifying | Resuming)
    }
}

impl fmt::Display for ConnectionStage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match *self {
            Self::Connected => "connected",
            Self::Connecting => "connecting",
            Self::Disconnected => "disconnected",
            Self::Handshake => "handshaking",
            Self::Identifying => "identifying",
            Self::Resuming => "resuming",
        })
    }
}

/// The type of reconnection that should be performed.
#[derive(Debug)]
#[non_exhaustive]
pub enum ReconnectType {
    /// Indicator that a new connection should be made by sending an IDENTIFY.
    Reidentify,
    /// Indicator that a new connection should be made by sending a RESUME.
    Resume,
}

/// [Discord docs](https://discord.com/developers/docs/topics/gateway-events#request-guild-members).
#[derive(Clone, Debug)]
pub enum ChunkGuildFilter {
    /// Returns all members of the guilds specified.
    None,
    /// A common username prefix filter for the members returned.
    ///
    /// Will return a maximum of 100 members.
    Query(String),
    /// A set of exact user IDs to query for.
    ///
    /// Will return a maximum of 100 members.
    UserIds(Vec<UserId>),
}
