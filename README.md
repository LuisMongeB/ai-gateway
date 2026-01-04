# 🚀 AI Gateway

A high-performance API gateway for LLM providers, built in Rust. Provides a unified OpenAI-compatible interface to multiple AI backends.

```
┌─────────────────────────────────────────────────────────────────┐
│                         AI Gateway                              │
│                                                                 │
│   Client ──► /v1/chat/completions ──┬──► Ollama (local)        │
│                                     │                           │
│              OpenAI-compatible      └──► OpenAI API             │
│                                                                 │
│   Features: Auth • Rate Limiting • Streaming • Logging          │
└─────────────────────────────────────────────────────────────────┘
```

## ✨ Features

- **🔄 Unified API** — OpenAI-compatible endpoint for all providers
- **🌊 Streaming Support** — Server-Sent Events (SSE) for real-time responses
- **🔌 Multiple Providers** — Ollama and OpenAI (more coming)
- **⚡ High Performance** — Built with Rust + Actix-web for low latency
- **🔧 Easy Configuration** — Environment-based provider selection

## 🏗️ Architecture

```
                         ┌─────────────┐
                         │    .env     │
                         │ AI_PROVIDER │
                         └──────┬──────┘
                                │
                                ▼
┌────────┐            ┌─────────────────┐            ┌──────────────┐
│ Client │ ──────────►│   AI Gateway    │───────────►│    Ollama    │
│        │  OpenAI    │                 │  Ollama    │  (local LLM) │
│        │  Format    │  • Auth         │  Format    └──────────────┘
│        │            │  • Rate Limit   │
│        │◄───────────│  • Transform    │───────────►┌──────────────┐
│        │    SSE     │  • Stream       │  OpenAI    │  OpenAI API  │
└────────┘            └─────────────────┘  Format    └──────────────┘
```

### Provider Comparison

| Aspect | Ollama | OpenAI |
|--------|--------|--------|
| Base URL | `localhost:11434` | `api.openai.com` |
| Auth | None (local) | Bearer token |
| Request Format | Transform needed | Pass-through |
| Streaming | NDJSON → SSE | SSE (native) |

## 🚀 Getting Started

### Prerequisites

- [Rust](https://rustup.rs/) 1.75+
- [Ollama](https://ollama.ai/) (optional, for local models)
- OpenAI API key (optional, for OpenAI provider)

### Installation

```bash
# Clone the repository
git clone https://github.com/yourusername/ai-gateway.git
cd ai-gateway

# Copy environment template
cp .env.example .env

# Build the project
cargo build --release
```

### Configuration

Edit `.env` to configure your gateway:

```bash
# Provider selection: "ollama" or "openai"
AI_PROVIDER=ollama

# Ollama configuration
OLLAMA_BASE_URL=http://localhost:11434

# OpenAI configuration
OPENAI_API_KEY=sk-your-key-here
OPENAI_BASE_URL=https://api.openai.com
```

### Running

```bash
# Development
RUST_LOG=info cargo run

# Production
RUST_LOG=info ./target/release/ai-gateway
```

The gateway starts at `http://localhost:8080`

## 📡 API Usage

### Chat Completions

```bash
# Non-streaming request
curl -X POST http://localhost:8080/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{
    "model": "gpt-4o-mini",
    "messages": [
      {"role": "user", "content": "Hello, how are you?"}
    ]
  }'
```

```bash
# Streaming request
curl -X POST http://localhost:8080/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{
    "model": "gpt-4o-mini",
    "messages": [
      {"role": "user", "content": "Tell me a story"}
    ],
    "stream": true
  }'
```

### Health Check

```bash
curl http://localhost:8080/health
```

### Response Format

Responses follow the OpenAI Chat Completions format:

```json
{
  "id": "chatcmpl-abc123",
  "object": "chat.completion",
  "created": 1704380400,
  "model": "gpt-4o-mini",
  "choices": [
    {
      "index": 0,
      "message": {
        "role": "assistant",
        "content": "Hello! I'm doing well, thank you for asking."
      },
      "finish_reason": "stop"
    }
  ],
  "usage": {
    "prompt_tokens": 12,
    "completion_tokens": 15,
    "total_tokens": 27
  }
}
```

## 📁 Project Structure

```
ai-gateway/
├── Cargo.toml
├── .env                  # Environment configuration
└── src/
    ├── main.rs           # Server setup, provider selection
    ├── handlers/
    │   ├── mod.rs
    │   └── chat.rs       # /v1/chat/completions endpoint
    ├── models/
    │   └── mod.rs        # Request/response structs
    ├── middleware/       # Auth, rate limiting (coming soon)
    └── providers/
        ├── mod.rs        # LLMProvider trait, ProviderError
        ├── ollama.rs     # Ollama provider
        └── openai.rs     # OpenAI provider
```

## 🛣️ Roadmap

| Phase | Description | Status |
|-------|-------------|--------|
| 1 | Basic proxy to Ollama | ✅ Complete |
| 2 | SSE streaming support | ✅ Complete |
| 3 | Multiple providers (Ollama + OpenAI) | ✅ Complete |
| 4 | Middleware (auth, rate limiting) | 🔄 In Progress |
| 5 | Resilience (fallbacks, caching, retries) | ⏳ Planned |
| 6 | Azure OpenAI support | ⏳ Planned |

## 🛠️ Tech Stack

- **Language:** Rust 🦀
- **Web Framework:** [Actix-web](https://actix.rs/) 4.x
- **HTTP Client:** [Reqwest](https://docs.rs/reqwest) with streaming
- **Async Runtime:** [Tokio](https://tokio.rs/)
- **Serialization:** [Serde](https://serde.rs/)

## 📄 License

Apache-2.0

---

<p align="center">
  Built with 🦀 and ☕
</p>
