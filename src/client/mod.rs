//! The Client contains information about a single bot's token, as well as event handlers.
//! Dispatching events to configured handlers and starting the connections are handled
//! directly via the client. In addition, the `http` module and `Cache` are also automatically
//! handled by the Client module for you.
//!
//! A [`Context`] is provided for every handler.
//!
//! The `http` module is the lower-level method of interacting with the Discord REST API.
//! Realistically, there should be little reason to use this yourself, as the Context will do this
//! for you. A possible use case of using the `http` module is if you do not have a Cache, for
//! purposes such as low memory requirements.
//!
//! Click [here][Client examples] for an example on how to use a `Client`.
//!
//! [Client examples]: Client#examples

mod context;
#[cfg(feature = "gateway")]
pub(crate) mod dispatch;
mod error;
#[cfg(feature = "gateway")]
mod event_handler;

use std::future::IntoFuture;
use std::sync::Arc;

use futures::future::BoxFuture;
use tokio::sync::{Mutex, RwLock};
use tracing::instrument;
use typemap_rev::{TypeMap, TypeMapKey};

pub use self::context::Context;
pub use self::error::Error as ClientError;
#[cfg(feature = "gateway")]
pub use self::event_handler::{EventHandler, FullEvent, RawEventHandler};
#[cfg(feature = "cache")]
pub use crate::cache::Cache;
#[cfg(feature = "cache")]
use crate::cache::Settings as CacheSettings;
#[cfg(feature = "framework")]
use crate::framework::Framework;
#[cfg(feature = "voice")]
use crate::gateway::VoiceGatewayManager;
use crate::gateway::{ActivityData, PresenceData};
use crate::http::Http;
use crate::internal::prelude::*;
use crate::model::id::ApplicationId;
use crate::model::user::OnlineStatus;

/// A builder implementing [`IntoFuture`] building a [`Client`] to interact with Discord.
#[cfg(feature = "gateway")]
#[must_use = "Builders do nothing unless they are awaited"]
pub struct ClientBuilder {
    data: TypeMap,
    http: Http,
    #[cfg(feature = "cache")]
    cache_settings: CacheSettings,
    #[cfg(feature = "framework")]
    framework: Option<Box<dyn Framework>>,
    #[cfg(feature = "voice")]
    voice_manager: Option<Arc<dyn VoiceGatewayManager>>,
    event_handlers: Vec<Arc<dyn EventHandler>>,
    raw_event_handlers: Vec<Arc<dyn RawEventHandler>>,
    presence: PresenceData,
}

#[cfg(feature = "gateway")]
impl ClientBuilder {
    fn _new(http: Http) -> Self {
        Self {
            data: TypeMap::new(),
            http,
            #[cfg(feature = "cache")]
            cache_settings: CacheSettings::default(),
            #[cfg(feature = "framework")]
            framework: None,
            #[cfg(feature = "voice")]
            voice_manager: None,
            event_handlers: vec![],
            raw_event_handlers: vec![],
            presence: PresenceData::default(),
        }
    }

    /// Construct a new builder to call methods on for the client construction. The `token` will
    /// automatically be prefixed "Bot " if not already.
    ///
    /// **Panic**: If you have enabled the `framework`-feature (on by default), you must specify a
    /// framework via the [`Self::framework`] method, otherwise awaiting the builder will cause a
    /// panic.
    pub fn new(token: impl AsRef<str>) -> Self {
        Self::_new(Http::new(token.as_ref()))
    }

    /// Construct a new builder with a [`Http`] instance to calls methods on for the client
    /// construction.
    ///
    /// **Panic**: If you have enabled the `framework`-feature (on by default), you must specify a
    /// framework via the [`Self::framework`] method, otherwise awaiting the builder will cause a
    /// panic.
    pub fn new_with_http(http: Http) -> Self {
        Self::_new(http)
    }

    /// Sets a token for the bot. If the token is not prefixed "Bot ", this method will
    /// automatically do so.
    pub fn token(mut self, token: impl AsRef<str>) -> Self {
        self.http = Http::new(token.as_ref());

        self
    }

    /// Gets the current token used for the [`Http`] client.
    pub fn get_token(&self) -> &str {
        self.http.token()
    }

    /// Sets the application id.
    pub fn application_id(self, application_id: ApplicationId) -> Self {
        self.http.set_application_id(application_id);

        self
    }

    /// Gets the application ID, if already initialized. See [`Self::application_id`] for more
    /// info.
    pub fn get_application_id(&self) -> Option<ApplicationId> {
        self.http.application_id()
    }

    /// Sets the entire [`TypeMap`] that will be available in [`Context`]s. A [`TypeMap`] must not
    /// be constructed manually: [`Self::type_map_insert`] can be used to insert one type at a
    /// time.
    pub fn type_map(mut self, type_map: TypeMap) -> Self {
        self.data = type_map;

        self
    }

    /// Gets the type map. See [`Self::type_map`] for more info.
    pub fn get_type_map(&self) -> &TypeMap {
        &self.data
    }

    /// Insert a single `value` into the internal [`TypeMap`] that will be available in
    /// [`Context::data`]. This method can be called multiple times in order to populate the
    /// [`TypeMap`] with `value`s.
    pub fn type_map_insert<T: TypeMapKey>(mut self, value: T::Value) -> Self {
        self.data.insert::<T>(value);

        self
    }

    /// Sets the settings of the cache. Refer to [`Settings`] for more information.
    ///
    /// [`Settings`]: CacheSettings
    #[cfg(feature = "cache")]
    pub fn cache_settings(mut self, settings: CacheSettings) -> Self {
        self.cache_settings = settings;
        self
    }

    /// Gets the cache settings. See [`Self::cache_settings`] for more info.
    #[cfg(feature = "cache")]
    pub fn get_cache_settings(&self) -> &CacheSettings {
        &self.cache_settings
    }

    /// Sets the command framework to be used. It will receive messages sent over the gateway and
    /// then consider - based on its settings - whether to dispatch a command.
    ///
    /// *Info*: If a reference to the framework is required for manual dispatch, you can implement
    /// [`Framework`] on [`Arc<YourFrameworkType>`] instead of `YourFrameworkType`.
    #[cfg(feature = "framework")]
    pub fn framework<F>(mut self, framework: F) -> Self
    where
        F: Framework + 'static,
    {
        self.framework = Some(Box::new(framework));

        self
    }

    /// Gets the framework, if already initialized. See [`Self::framework`] for more info.
    #[cfg(feature = "framework")]
    pub fn get_framework(&self) -> Option<&dyn Framework> {
        self.framework.as_deref()
    }

    /// Sets the voice gateway handler to be used. It will receive voice events sent over the
    /// gateway and then consider - based on its settings - whether to dispatch a command.
    ///
    /// *Info*: If a reference to the voice_manager is required for manual dispatch, use the
    /// [`Self::voice_manager_arc`]-method instead.
    #[cfg(feature = "voice")]
    pub fn voice_manager<V>(mut self, voice_manager: V) -> Self
    where
        V: VoiceGatewayManager + 'static,
    {
        self.voice_manager = Some(Arc::new(voice_manager));

        self
    }

    /// This method allows to pass an [`Arc`]'ed `voice_manager` - this step is done for you in the
    /// [`voice_manager`]-method, if you don't need the extra control. You can provide a clone and
    /// keep the original to manually dispatch.
    ///
    /// [`voice_manager`]: Self::voice_manager
    #[cfg(feature = "voice")]
    pub fn voice_manager_arc(
        mut self,
        voice_manager: Arc<dyn VoiceGatewayManager + 'static>,
    ) -> Self {
        self.voice_manager = Some(voice_manager);

        self
    }

    /// Gets the voice manager, if already initialized. See [`Self::voice_manager`] for more info.
    #[cfg(feature = "voice")]
    pub fn get_voice_manager(&self) -> Option<Arc<dyn VoiceGatewayManager>> {
        self.voice_manager.clone()
    }

    /// Adds an event handler with multiple methods for each possible event.
    pub fn event_handler<H: EventHandler + 'static>(mut self, event_handler: H) -> Self {
        self.event_handlers.push(Arc::new(event_handler));

        self
    }

    /// Adds an event handler with multiple methods for each possible event. Passed by Arc.
    pub fn event_handler_arc<H: EventHandler + 'static>(
        mut self,
        event_handler_arc: Arc<H>,
    ) -> Self {
        self.event_handlers.push(event_handler_arc);

        self
    }

    /// Gets the added event handlers. See [`Self::event_handler`] for more info.
    pub fn get_event_handlers(&self) -> &[Arc<dyn EventHandler>] {
        &self.event_handlers
    }

    /// Adds an event handler with a single method where all received gateway events will be
    /// dispatched.
    pub fn raw_event_handler<H: RawEventHandler + 'static>(mut self, raw_event_handler: H) -> Self {
        self.raw_event_handlers.push(Arc::new(raw_event_handler));

        self
    }

    /// Gets the added raw event handlers. See [`Self::raw_event_handler`] for more info.
    pub fn get_raw_event_handlers(&self) -> &[Arc<dyn RawEventHandler>] {
        &self.raw_event_handlers
    }

    /// Sets the initial activity.
    pub fn activity(mut self, activity: ActivityData) -> Self {
        self.presence.activity = Some(activity);

        self
    }

    /// Sets the initial status.
    pub fn status(mut self, status: OnlineStatus) -> Self {
        self.presence.status = status;

        self
    }

    /// Gets the initial presence. See [`Self::activity`] and [`Self::status`] for more info.
    pub fn get_presence(&self) -> &PresenceData {
        &self.presence
    }
}

#[cfg(feature = "gateway")]
impl IntoFuture for ClientBuilder {
    type Output = Result<Client>;

    type IntoFuture = BoxFuture<'static, Result<Client>>;

    #[instrument(skip(self))]
    fn into_future(self) -> Self::IntoFuture {
        let data = Arc::new(RwLock::new(self.data));
        #[cfg(feature = "framework")]
        let framework = self.framework;
        let event_handlers = self.event_handlers;

        let mut http = self.http;

        if let Some(ratelimiter) = &mut http.ratelimiter {
            let event_handlers_clone = event_handlers.clone();
            ratelimiter.set_ratelimit_callback(Box::new(move |info| {
                for event_handler in event_handlers_clone.iter().map(Arc::clone) {
                    let info = info.clone();
                    tokio::spawn(async move { event_handler.ratelimit(info).await });
                }
            }));
        }

        let http = Arc::new(http);

        #[cfg(feature = "voice")]
        let voice_manager = self.voice_manager;

        #[cfg(feature = "cache")]
        let cache = Arc::new(Cache::new_with_settings(self.cache_settings));

        Box::pin(async move {
            let ws_url = Arc::new(Mutex::new(match http.get_gateway().await {
                Ok(response) => response.url,
                Err(err) => {
                    tracing::warn!("HTTP request to get gateway URL failed: {}", err);
                    "wss://gateway.discord.gg".to_string()
                },
            }));

            let client = Client {
                data,
                #[cfg(feature = "voice")]
                voice_manager,
                ws_url,
                #[cfg(feature = "cache")]
                cache,
                http,
            };
            #[cfg(feature = "framework")]
            if let Some(mut framework) = framework {
                framework.init(&client).await;
            }
            Ok(client)
        })
    }
}

/// The Client is the way to be able to start sending authenticated requests over the REST API, as
/// well as initializing a WebSocket connection.
///
/// # Event Handlers
///
/// Event handlers can be configured. For example, the event handler [`EventHandler::message`] will
/// be dispatched to whenever a [`Event::MessageCreate`] is received over the connection.
///
/// Note that you do not need to manually handle events, as they are handled internally and then
/// dispatched to your event handlers.
///
/// # Examples
///
/// Creating a Client instance and adding a handler on every message receive, acting as a
/// "ping-pong" bot is simple:
///
/// ```no_run
/// use serenity::model::prelude::*;
/// use serenity::prelude::*;
/// use serenity::Client;
///
/// struct Handler;
///
/// #[serenity::async_trait]
/// impl EventHandler for Handler {
///     async fn message(&self, context: Context, msg: Message) {
///         if msg.content == "!ping" {
///             let _ = msg.channel_id.say(&context, "Pong!");
///         }
///     }
/// }
///
/// # async fn run() -> Result<(), Box<dyn std::error::Error>> {
/// let mut client =
///     Client::builder("my token here", GatewayIntents::default()).event_handler(Handler).await?;
///
/// client.start().await?;
/// # Ok(())
/// # }
/// ```
///
/// [`Event::MessageCreate`]: crate::model::event::Event::MessageCreate
#[cfg(feature = "gateway")]
pub struct Client {
    /// A TypeMap which requires types to be Send + Sync. This is a map that can be safely shared
    /// across contexts.
    ///
    /// The purpose of the data field is to be accessible and persistent across contexts; that is,
    /// data can be modified by one context, and will persist through the future and be accessible
    /// through other contexts. This is useful for anything that should "live" through the program:
    /// counters, database connections, custom user caches, etc.
    ///
    /// In the meaning of a context, this data can be accessed through [`Context::data`].
    ///
    /// # Examples
    ///
    /// Create a `MessageEventCounter` to track the following events:
    ///
    /// - [`Event::MessageCreate`]
    /// - [`Event::MessageDelete`]
    /// - [`Event::MessageDeleteBulk`]
    /// - [`Event::MessageUpdate`]
    ///
    /// ```rust,ignore
    /// use std::collections::HashMap;
    /// use std::env;
    ///
    /// use serenity::model::prelude::*;
    /// use serenity::prelude::*;
    ///
    /// struct MessageEventCounter;
    ///
    /// impl TypeMapKey for MessageEventCounter {
    ///     type Value = HashMap<String, u64>;
    /// }
    ///
    /// async fn reg<S: Into<String>>(ctx: Context, name: S) {
    ///     let mut data = ctx.data.write().await;
    ///     let counter = data.get_mut::<MessageEventCounter>().unwrap();
    ///     let entry = counter.entry(name.into()).or_insert(0);
    ///     *entry += 1;
    /// }
    ///
    /// struct Handler;
    ///
    /// #[serenity::async_trait]
    /// impl EventHandler for Handler {
    ///     async fn message(&self, ctx: Context, _: Message) {
    ///         reg(ctx, "MessageCreate").await
    ///     }
    ///     async fn message_delete(&self, ctx: Context, _: ChannelId, _: MessageId) {
    ///         reg(ctx, "MessageDelete").await
    ///     }
    ///     async fn message_delete_bulk(&self, ctx: Context, _: ChannelId, _: Vec<MessageId>) {
    ///         reg(ctx, "MessageDeleteBulk").await
    ///     }
    ///
    ///     #[cfg(feature = "cache")]
    ///     async fn message_update(
    ///         &self,
    ///         ctx: Context,
    ///         _old: Option<Message>,
    ///         _new: Option<Message>,
    ///         _: MessageUpdateEvent,
    ///     ) {
    ///         reg(ctx, "MessageUpdate").await
    ///     }
    ///
    ///     #[cfg(not(feature = "cache"))]
    ///     async fn message_update(&self, ctx: Context, _new_data: MessageUpdateEvent) {
    ///         reg(ctx, "MessageUpdate").await
    ///     }
    /// }
    ///
    /// # async fn run() -> Result<(), Box<dyn std::error::Error>> {
    /// let token = std::env::var("DISCORD_TOKEN")?;
    /// let mut client = Client::builder(&token, GatewayIntents::default()).event_handler(Handler).await?;
    /// {
    ///     let mut data = client.data.write().await;
    ///     data.insert::<MessageEventCounter>(HashMap::default());
    /// }
    ///
    /// client.start().await?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// Refer to [example 05] for an example on using the [`Self::data`] field.
    ///
    /// [`Event::MessageCreate`]: crate::model::event::Event::MessageCreate
    /// [`Event::MessageDelete`]: crate::model::event::Event::MessageDelete
    /// [`Event::MessageDeleteBulk`]: crate::model::event::Event::MessageDeleteBulk
    /// [`Event::MessageUpdate`]: crate::model::event::Event::MessageUpdate
    /// [example 05]: https://github.com/serenity-rs/serenity/tree/current/examples/e05_command_framework
    pub data: Arc<RwLock<TypeMap>>,
    /// The voice manager for the client.
    ///
    /// This is an ergonomic structure for interfacing over voice
    /// connections.
    #[cfg(feature = "voice")]
    pub voice_manager: Option<Arc<dyn VoiceGatewayManager + 'static>>,
    /// URL that the client's will use to connect to the gateway.
    ///
    /// This is likely not important for production usage and is, at best, used for debugging.
    ///
    /// This is wrapped in an `Arc<Mutex<T>>` so it will have an updated value available.
    pub ws_url: Arc<Mutex<String>>,
    /// The cache for the client.
    #[cfg(feature = "cache")]
    pub cache: Arc<Cache>,
    /// An HTTP client.
    pub http: Arc<Http>,
}

impl Client {
    pub fn builder(token: impl AsRef<str>) -> ClientBuilder {
        ClientBuilder::new(token)
    }

    /// Establish the connection and start listening for events.
    ///
    /// This will start receiving events in a loop and start dispatching the events to your
    /// registered handlers.
    ///
    /// # Examples
    ///
    /// Starting a Client:
    ///
    /// ```rust,no_run
    /// # use std::error::Error;
    /// # use serenity::prelude::*;
    /// use serenity::Client;
    ///
    /// # async fn run() -> Result<(), Box<dyn Error>> {
    /// let token = std::env::var("DISCORD_TOKEN")?;
    /// let mut client = Client::builder(&token, GatewayIntents::default()).await?;
    ///
    /// if let Err(why) = client.start().await {
    ///     println!("Err with client: {:?}", why);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    #[instrument(skip(self))]
    pub async fn start(&mut self) -> Result<()> {
        self.start_connection().await
    }
    
    /// # Errors
    ///
    /// Returns a [`ClientError::Shutdown`] when the client shuts down due to an error.
    #[instrument(skip(self))]
    async fn start_connection(
        &mut self,
    ) -> Result<()> {
        #[cfg(feature = "voice")]
        if let Some(voice_manager) = &self.voice_manager {
            let user = self.http.get_current_user().await?;

            voice_manager.initialise(user.id).await;
        }

        Ok(())
    }
}
