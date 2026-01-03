use std::pin::Pin;
use std::fmt;
use futures::Stream;
use bytes::Bytes;
use async_trait::async_trait;
pub mod ollama;
pub mod openai;

use crate::models::{ChatCompletionRequest, ChatCompletionResponse};

#[derive(Debug)]
pub enum ProviderError {
    Network(String),
    Parse(String),
    ProviderError {
        status: u16,
        message: String,
    },
}

impl fmt::Display for ProviderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProviderError::Network(msg) => write!(f, "Network error: {}", msg),
            ProviderError::Parse(msg) => write!(f, "Parse error: {}", msg),
            ProviderError::ProviderError { status, message } => {
                write!(f, "Provider error ({}): {}", status, message)
            }
        }
    }
}

impl std::error::Error for ProviderError {}

#[async_trait]
pub trait LLMProvider: Send + Sync {
    async fn chat(&self, req: ChatCompletionRequest) -> Result<ChatCompletionResponse, ProviderError>;
    async fn chat_stream(&self, req: ChatCompletionRequest) -> Result<Pin<Box<dyn Stream<Item = Result<Bytes, ProviderError>> + Send>>, ProviderError>;
}

