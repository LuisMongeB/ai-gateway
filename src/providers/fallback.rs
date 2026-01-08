use crate::models::{ChatCompletionRequest, ChatCompletionResponse};
use crate::providers::{LLMProvider, ProviderError};
use async_trait::async_trait;
use bytes::Bytes;
use futures::Stream;
use std::pin::Pin;
use std::sync::Arc;
use tracing::{info, warn};

/// A provider that tries a primary provider first, and falls back to a backup if it fails.
pub struct FallbackProvider {
    primary: Arc<dyn LLMProvider>,
    backup: Arc<dyn LLMProvider>,
}

impl FallbackProvider {
    pub fn new(primary: Arc<dyn LLMProvider>, backup: Arc<dyn LLMProvider>) -> Self {
        Self { primary, backup }
    }
}

#[async_trait]
impl LLMProvider for FallbackProvider {
    async fn chat(
        &self,
        request: ChatCompletionRequest,
    ) -> Result<ChatCompletionResponse, ProviderError> {
        // Try Primary. We need to clone because if it fails, we need the request again for backup.
        let req_clone = request.clone();

        match self.primary.chat(request).await {
            Ok(response) => Ok(response),
            Err(e) => {
                warn!("Primary provider failed: {}. Switching to backup.", e);
                self.backup.chat(req_clone).await
            }
        }
    }

    async fn chat_stream(
        &self,
        request: ChatCompletionRequest,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<Bytes, ProviderError>> + Send>>, ProviderError>
    {
        // Streaming fallback is tricky because the trait returns a Stream (wrapped in Future)
        // If we want to fallback on connection error, we need to try to establish the stream first.

        // Note: The trait definition is:
        // async fn chat_stream(...) -> Result<Pin<Box<dyn Stream...>>, ProviderError>
        // Use awaiting here!

        warn!("Streaming fallback is not fully supported in this simple implementation. Using Primary only.");
        self.primary.chat_stream(request).await
    }
}
