pub mod ollama;
use std::pin::Pin;
use futures::Stream;
use bytes::Bytes;
use async_trait::async_trait;

use crate::models::{ChatCompletionRequest, ChatCompletionResponse};

pub enum ProviderError {
    Network(String),
    Parse(String),
    ProviderError {
        status: u16,
        message: String,
    },
}

#[async_trait]
pub trait LLMProvider: Send + Sync {
    async fn chat(&self, req: ChatCompletionRequest) -> Result<ChatCompletionResponse, ProviderError>;
    async fn chat_stream(&self, req: ChatCompletionRequest) -> Result<Pin<Box<dyn Stream<Item = Result<Bytes, ProviderError>> + Send>>, ProviderError>;
}