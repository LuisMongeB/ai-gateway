use crate::models::{
    ChatCompletionChunk, ChatCompletionRequest, ChatCompletionResponse, Choice, ChunkChoice, Delta,
    OllamaRequest, OllamaResponse, OllamaStreamChunk, Usage,
};
use crate::providers::{LLMProvider, ProviderError};
use async_trait::async_trait;
use bytes::Bytes;
use futures::{Stream, StreamExt};
use log::info;
use reqwest::Client;
use std::pin::Pin;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

pub struct OllamaProvider {
    client: Client,
    base_url: String,
}

impl OllamaProvider {
    pub fn new(base_url: String) -> Self {
        let client = Client::new();

        Self { client, base_url }
    }
}

#[async_trait]
impl LLMProvider for OllamaProvider {
    async fn chat(
        &self,
        req: ChatCompletionRequest,
    ) -> Result<ChatCompletionResponse, ProviderError> {
        info!("Processing request...");
        let ollama_request = OllamaRequest {
            model: req.model,
            messages: req.messages,
            stream: false,
        };

        let response = self
            .client
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

    async fn chat_stream(
        &self,
        req: ChatCompletionRequest,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<Bytes, ProviderError>> + Send>>, ProviderError>
    {
        let ollama_request = OllamaRequest {
            model: req.model.clone(),
            messages: req.messages,
            stream: true,
        };

        info!("Calling provider...");
        let response = self
            .client
            .post(format!("{}/api/chat", self.base_url))
            .json(&ollama_request)
            .send()
            .await
            .map_err(|e| ProviderError::Network(e.to_string()))?;

        let response_id = format!("chatcmpl-{}", Uuid::new_v4());
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let model_name = req.model;

        let sse_stream = async_stream::stream! {
            let mut byte_stream = response.bytes_stream();

            while let Some(chunk_result) = byte_stream.next().await {
                match chunk_result {
                    Ok(bytes) => {
                        let text = String::from_utf8_lossy(&bytes);

                        for line in text.lines() {
                            if line.trim().is_empty() {
                                continue;
                            }

                            match serde_json::from_str::<OllamaStreamChunk>(line) {
                                Ok(ollama_chunk) => {
                                    if ollama_chunk.message.content.is_empty() && !ollama_chunk.done {
                                        continue;
                                    }

                                    let openai_chunk = ChatCompletionChunk {
                                        id: response_id.clone(),
                                        object: String::from("chat.completion.chunk"),
                                        created: timestamp,
                                        model: model_name.clone(),
                                        choices: vec![ChunkChoice {
                                            index: 0,
                                            delta: Delta {
                                                role: None,
                                                content: ollama_chunk.message.content,
                                            },
                                            finish_reason: if ollama_chunk.done {
                                                Some(String::from("stop"))
                                            } else {
                                                None
                                            },
                                        }],
                                        usage: if ollama_chunk.done {
                                            Some(Usage {
                                                prompt_tokens: ollama_chunk.prompt_eval_count.unwrap_or(0),
                                                completion_tokens: ollama_chunk.eval_count.unwrap_or(0),
                                                total_tokens: ollama_chunk.prompt_eval_count.unwrap_or(0) + ollama_chunk.eval_count.unwrap_or(0),
                                            })
                                        } else {
                                            None
                                        },
                                    };

                                    let json = serde_json::to_string(&openai_chunk).unwrap();
                                    let sse_event = format!("data: {}\n\n", json);
                                    yield Ok::<_, ProviderError>(Bytes::from(sse_event));
                                }
                                Err(e) => {
                                    info!("Failed to parse chunk: {}", e);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        info!("Stream error: {}", e);
                        break;
                    }
                }
            }
            yield Ok::<_, ProviderError>(Bytes::from("data: [DONE]\n\n"));
        };

        Ok(Box::pin(sse_stream))
    }
}
