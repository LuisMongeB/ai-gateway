use std::pin::Pin;
use futures::{Stream, StreamExt};
use bytes::Bytes;
use reqwest::Client;
use async_trait::async_trait;
use crate::models::{ChatCompletionRequest, ChatCompletionResponse};
use crate::providers::{LLMProvider, ProviderError};
use log::info;

#[derive(Clone)]
pub struct OpenAIProvider {
    client: reqwest::Client,
    base_url: String,
    api_key: String,
}

impl OpenAIProvider {
    pub fn new(base_url: String, api_key: String) -> Self {
        let client = Client::new();

        Self {
            client,
            base_url,
            api_key,
        }
    }
}

#[async_trait]
impl LLMProvider for OpenAIProvider {
    async fn chat(&self, req: ChatCompletionRequest) -> Result<ChatCompletionResponse, ProviderError> {
        info!("Processing request to OpenAI...");

        let response = self.client
            .post(format!("{}/v1/chat/completions", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&req)
            .send()
            .await
            .map_err(|e| ProviderError::Network(e.to_string()))?;

        let openai_response = response
            .json::<ChatCompletionResponse>()
            .await
            .map_err(|e| ProviderError::Parse(e.to_string()))?;

        info!("Request processed successfully");
        Ok(openai_response)
    }

    async fn chat_stream(&self, req: ChatCompletionRequest) -> Result<Pin<Box<dyn Stream<Item = Result<Bytes, ProviderError>> + Send>>, ProviderError> {
        info!("Processing streaming request to OpenAI...");

        let response = self.client
            .post(format!("{}/v1/chat/completions", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&req)
            .send()
            .await
            .map_err(|e| ProviderError::Network(e.to_string()))?;

        let stream = async_stream::stream! {
            let mut byte_stream = response.bytes_stream();

            while let Some(chunk_result) = byte_stream.next().await {
                match chunk_result {
                    Ok(bytes) => {
                        yield Ok(bytes);
                    }
                    Err(e) => {
                        info!("Stream error: {}", e);
                        yield Err(ProviderError::Network(e.to_string()));
                        break;
                    }
                }
            }
        };
        Ok(Box::pin(stream))
    }
}

