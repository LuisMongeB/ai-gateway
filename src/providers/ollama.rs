use async_trait::async_trait;
use std::pin::Pin;
use futures::Stream;
use bytes::Bytes;
use crate::models::{
    ChatCompletionRequest, ChatCompletionResponse,
    OllamaRequest, OllamaResponse,
    Choice, Usage
};
use crate::providers::{LLMProvider, ProviderError};
use uuid::Uuid;
use std::time::{SystemTime, UNIX_EPOCH};
use reqwest::Client;
use log::info;

pub struct OllamaProvider {
    client: reqwest::Client,
    base_url: String,
}

impl OllamaProvider {
    pub fn new(base_url: String) -> Self {
        let client = reqwest::Client::new();
        
        Self {
            client,
            base_url,
        }
    }
}

#[async_trait]
impl LLMProvider for OllamaProvider {
    async fn chat(&self, req: ChatCompletionRequest) -> Result<ChatCompletionResponse, ProviderError> {
        info!("Processing request...");
        let ollama_request = OllamaRequest {
            model: req.model,
            messages: req.messages,
            stream: false,
        };
    
        let response = self.client
                .post(format!("{}/api/chat", self.base_url)) // "http://localhost:11434/api/chat"
                .json(&ollama_request)
                .send()
                .await;
    
        let ollama_response = match response {
            Ok(resp) => resp,
            Err(e) => {
                return Err(ProviderError::Network(e.to_string()));
            }
        };
    
        let ollama_data = match ollama_response.json::<OllamaResponse>().await {
            Ok(data) => data,
            Err(e) => {
                return Err(ProviderError::Parse(e.to_string()));
            }
        };
    
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
    
        let chat_completion_response = ChatCompletionResponse {
            id: format!("chatcmpl-{}", Uuid::new_v4()),
            object: String::from("chat.completion"),
            created: timestamp,
            model: ollama_data.model,
            choices: vec![Choice {
                index: 0,
                message: ollama_data.message,
                finish_reason: String::from("stop"),
            }],
            usage: Usage {
                prompt_tokens: ollama_data.prompt_eval_count,
                completion_tokens: ollama_data.eval_count,
                total_tokens: ollama_data.prompt_eval_count + ollama_data.eval_count,
            },
        };
        
        info!("Request has been processed successfully");
        Ok(chat_completion_response)
    
    }

    async fn chat_stream(&self, req: ChatCompletionRequest) -> Result<Pin<Box<dyn Stream<Item = Result<Bytes, ProviderError>> + Send>>, ProviderError> {
        todo!()
    }
}

