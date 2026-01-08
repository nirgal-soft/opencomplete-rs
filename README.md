# opencomplete

A local daemon that brings AI code completions to your editor.

```
fn main() {
    let numbers = vec![1, 2, 3, 4, 5];
    let sum: i32 = numbers.iter()█
}
```
↓
```rust
    let sum: i32 = numbers.iter().sum();
```

## Features

- **Multiple providers** — Claude API or local models via Ollama
- **FIM support** — Native fill-in-the-middle for Qwen, DeepSeek, StarCoder, CodeLlama
- **Fast** — Keeps models warm, minimal overhead
- **Simple HTTP API** — Easy to integrate with any editor

## Quick Start

```bash
# Build
cargo build --release

# Run (starts on port 8642)
./target/release/opencomplete-rs
```

## Configuration

Config lives at `~/.config/opencomplete/config.toml`:

```toml
default_provider = "ollama"

[server]
port = 8642
host = "127.0.0.1"

# Local models via Ollama (free, private)
[[providers]]
type = "ollama"
model = "qwen3-coder:latest"
base_url = "http://localhost:11434"

# Claude API (requires ANTHROPIC_API_KEY)
[[providers]]
type = "claude"
model = "claude-sonnet-4-20250514"
```

## API

### `POST /complete`

```bash
curl -X POST http://localhost:8642/complete \
  -H "Content-Type: application/json" \
  -d '{
    "prefix": "fn main() {\n    ",
    "suffix": "\n}",
    "language": "rust",
    "max_tokens": 128
  }'
```

Response (SSE):
```
data: {"text":"println!(\"Hello, world!\");","is_final":true,"finish_reason":"stop"}
```

### `GET /health`

```json
{
  "status": "ok",
  "version": "0.1.0",
  "providers": [
    {"id": "ollama", "name": "Ollama (Local)", "is_default": true},
    {"id": "claude", "name": "Claude (Anthropic)", "is_default": false}
  ]
}
```

## Request Schema

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `prefix` | string | yes | Code before cursor |
| `suffix` | string | yes | Code after cursor |
| `language` | string | yes | Language identifier |
| `max_tokens` | number | no | Max tokens to generate (default: 256) |
| `style_hints` | string | no | Formatting hints, e.g. `"tabwidth=2, trailing commas"` |

## Supported Models

**Ollama (local):**
- `qwen3-coder` — Recommended, excellent FIM support
- `deepseek-coder`
- `codellama`
- `starcoder2`

**Claude API:**
- `claude-sonnet-4-20250514`
- `claude-3-5-haiku-20241022`
- `claude-opus-4-20250514`

## License

MIT
