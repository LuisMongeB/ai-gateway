use actix_web::{web, Responder, HttpResponse};
use crate::models::{
    ChatCompletionRequest,
    ChatCompletionResponse,
    OllamaRequest,
    OllamaResponse,
    Choice,
    Usage,
    OllamaStreamChunk,
    ChatCompletionChunk,
    ChunkChoice,
    Delta
};
use uuid::Uuid;
use log::info;
use futures_util::StreamExt;
use async_stream::stream;

use std::{time::{SystemTime, UNIX_EPOCH}};

pub async fn chat_completions(
    client: web::Data<reqwest::Client>,
    body: web::Json<ChatCompletionRequest>,
) -> impl Responder {
    let request = body.into_inner();

    let is_streaming = request.stream.unwrap_or(false);

    if is_streaming {
        info!("Streaming request received");
    
        let ollama_request = OllamaRequest {
            model: request.model.clone(),  // clone because we need it later
            messages: request.messages,
            stream: true,
        };
    
        let response = client
            .post("http://localhost:11434/api/chat")
            .json(&ollama_request)
            .send()
            .await;
    
        let ollama_response = match response {
            Ok(resp) => resp,
            Err(e) => {
                return HttpResponse::InternalServerError().body(format!("Ollama request failed: {}", e));
            }
        };
    
        // Prepare values needed for all chunks
        let response_id = format!("chatcmpl-{}", Uuid::new_v4());
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let model_name = request.model;
        
        let sse_stream = async_stream::stream! {
            let mut stream = ollama_response.bytes_stream();

            while let Some(chunk_result) = stream.next().await {
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
                                    };

                                    let json = serde_json::to_string(&openai_chunk).unwrap();
                                    let sse_event = format!("data: {}\n\n", json);
                                    yield Ok::<_, std::io::Error>(actix_web::web::Bytes::from(sse_event));
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

            yield Ok::<_, std::io::Error>(actix_web::web::Bytes::from("data: [DONE]\n\n"));
        };

        return HttpResponse::Ok()
        .content_type("text/event-stream")
        .streaming(sse_stream)
    }

    info!("Processing request...");
    let ollama_request = OllamaRequest {
        model: request.model,
        messages: request.messages,
        stream: false,
    };

    let response = client
            .post("http://localhost:11434/api/chat")
            .json(&ollama_request)
            .send()
            .await;

    let ollama_response = match response {
        Ok(resp) => resp,
        Err(e) => {
            return HttpResponse::InternalServerError().json(e.to_string())
        }
    };

    let ollama_data = match ollama_response.json::<OllamaResponse>().await {
        Ok(data) => data,
        Err(e) => {
            return HttpResponse::InternalServerError().body(format!("Failed to parse response: {}", e))
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

    HttpResponse::Ok().json(chat_completion_response)

}
