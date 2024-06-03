#[cfg(feature = "http")]
use super::{check_overflow, Builder};
use super::{
    CreateActionRow,
    CreateAllowedMentions,
    CreateAttachment,
    CreateEmbed,
    EditAttachments,
};
#[cfg(feature = "http")]
use crate::constants;
#[cfg(feature = "http")]
use crate::http::CacheHttp;
#[cfg(feature = "http")]
use crate::internal::prelude::*;
use crate::model::prelude::*;

/// A builder to specify the fields to edit in an existing [`Webhook`]'s message.
///
/// [Discord docs](https://discord.com/developers/docs/resources/webhook#edit-webhook-message)
#[derive(Clone, Debug, Default, Serialize)]
#[must_use]
pub struct EditWebhookMessage {
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    embeds: Option<Vec<CreateEmbed>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    allowed_mentions: Option<CreateAllowedMentions>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) components: Option<Vec<CreateActionRow>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) attachments: Option<EditAttachments>,

    #[serde(skip)]
    thread_id: Option<ChannelId>,
}

impl EditWebhookMessage {
    /// Equivalent to [`Self::default`].
    pub fn new() -> Self {
        Self::default()
    }

    #[cfg(feature = "http")]
    pub(crate) fn check_length(&self) -> Result<()> {
        if let Some(content) = &self.content {
            check_overflow(content.chars().count(), constants::MESSAGE_CODE_LIMIT)
                .map_err(|overflow| Error::Model(ModelError::MessageTooLong(overflow)))?;
        }

        if let Some(embeds) = &self.embeds {
            check_overflow(embeds.len(), constants::EMBED_MAX_COUNT)
                .map_err(|_| Error::Model(ModelError::EmbedAmount))?;
            for embed in embeds {
                embed.check_length()?;
            }
        }

        Ok(())
    }

    /// Set the content of the message.
    ///
    /// **Note**: Message contents must be under 2000 unicode code points.
    #[inline]
    pub fn content(mut self, content: impl Into<String>) -> Self {
        self.content = Some(content.into());
        self
    }

    /// Edits a message within a given thread. If the provided thread Id doesn't belong to the
    /// current webhook, the API will return an error.
    #[inline]
    pub fn in_thread(mut self, thread_id: impl Into<ChannelId>) -> Self {
        self.thread_id = Some(thread_id.into());
        self
    }

    /// Adds an embed for the message.
    ///
    /// Embeds from the original message are reset when adding new embeds and must be re-added.
    pub fn add_embed(mut self, embed: CreateEmbed) -> Self {
        self.embeds.get_or_insert(Vec::new()).push(embed);
        self
    }

    /// Adds multiple embeds to the message.
    ///
    /// Embeds from the original message are reset when adding new embeds and must be re-added.
    pub fn add_embeds(mut self, embeds: Vec<CreateEmbed>) -> Self {
        self.embeds.get_or_insert(Vec::new()).extend(embeds);
        self
    }

    /// Sets a single embed to include in the message
    ///
    /// Calling this will overwrite the embed list. To append embeds, call [`Self::add_embed`]
    /// instead.
    pub fn embed(mut self, embed: CreateEmbed) -> Self {
        self.embeds = Some(vec![embed]);
        self
    }

    /// Sets the embeds for the message.
    ///
    /// **Note**: You can only have up to 10 embeds per message.
    ///
    /// Calling this will overwrite the embed list. To append embeds, call [`Self::add_embeds`]
    /// instead.
    pub fn embeds(mut self, embeds: Vec<CreateEmbed>) -> Self {
        self.embeds = Some(embeds);
        self
    }

    /// Set the allowed mentions for the message.
    pub fn allowed_mentions(mut self, allowed_mentions: CreateAllowedMentions) -> Self {
        self.allowed_mentions = Some(allowed_mentions);
        self
    }

    /// Sets the components for this message. Requires an application-owned webhook, meaning either
    /// the webhook's `kind` field is set to [`WebhookType::Application`], or it was created by an
    /// application (and has kind [`WebhookType::Incoming`]).
    ///
    /// [`WebhookType::Application`]: crate::model::webhook::WebhookType
    /// [`WebhookType::Incoming`]: crate::model::webhook::WebhookType
    pub fn components(mut self, components: Vec<CreateActionRow>) -> Self {
        self.components = Some(components);
        self
    }
    super::button_and_select_menu_convenience_methods!(self.components);

    /// Sets attachments, see [`EditAttachments`] for more details.
    pub fn attachments(mut self, attachments: EditAttachments) -> Self {
        self.attachments = Some(attachments);
        self
    }

    /// Adds a new attachment to the message.
    ///
    /// Resets existing attachments. See the documentation for [`EditAttachments`] for details.
    pub fn new_attachment(mut self, attachment: CreateAttachment) -> Self {
        let attachments = self.attachments.get_or_insert_with(Default::default);
        self.attachments = Some(std::mem::take(attachments).add(attachment));
        self
    }

    /// Shorthand for [`EditAttachments::keep`].
    pub fn keep_existing_attachment(mut self, id: AttachmentId) -> Self {
        let attachments = self.attachments.get_or_insert_with(Default::default);
        self.attachments = Some(std::mem::take(attachments).keep(id));
        self
    }

    /// Shorthand for calling [`Self::attachments`] with [`EditAttachments::new`].
    pub fn clear_attachments(mut self) -> Self {
        self.attachments = Some(EditAttachments::new());
        self
    }
}

#[cfg(feature = "http")]
#[async_trait::async_trait]
impl Builder for EditWebhookMessage {
    type Context<'ctx> = (WebhookId, &'ctx str, MessageId);
    type Built = Message;

    /// Edits the webhook's message.
    ///
    /// **Note**: Message contents must be under 2000 unicode code points, and embeds must be under
    /// 6000 code points.
    ///
    /// # Errors
    ///
    /// Returns an [`Error::Model`] if the message content is too long.
    ///
    /// May also return an [`Error::Http`] if the content is malformed, the webhook's token is
    /// invalid, or the given message Id does not belong to the webhook.
    ///
    /// Or may return an [`Error::Json`] if there is an error deserialising Discord's response.
    async fn execute(
        mut self,
        cache_http: impl CacheHttp,
        ctx: Self::Context<'_>,
    ) -> Result<Self::Built> {
        self.check_length()?;

        let files = self.attachments.as_mut().map_or(Vec::new(), |a| a.take_files());

        let http = cache_http.http();
        if self.allowed_mentions.is_none() {
            self.allowed_mentions.clone_from(&http.default_allowed_mentions);
        }

        http.edit_webhook_message(ctx.0, self.thread_id, ctx.1, ctx.2, &self, files).await
    }
}
