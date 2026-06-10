# Duihua Gateway

OpenAI-compatible HTTP gateway for self-hosted inference. Proxies `/v1/models`, `/v1/chat/completions`, `/v1/messages`, `/v1/embeddings`, and `/v1/responses` to upstream model runtimes and optionally persists Responses API state via [responses-api-store](https://github.com/beranekio/responses-api-store).

This repository was extracted from [beranekio/duihua-ai-services](https://github.com/beranekio/duihua-ai-services) so the gateway can be developed and released independently.

## Repository layout

| Path | Purpose |
| --- | --- |
| `src/` | Rust gateway service (Axum) |
| `charts/duihua-gateway/` | Helm subchart for Kubernetes deployment |
| `scripts/helm-smoke-kind.sh` | Local/CI Helm smoke test against kind |
| `Dockerfile` | Multi-stage build: `rust:1-bookworm` builder, distroless runtime |

## API surface

| Endpoint | Method | Description |
| --- | --- | --- |
| `/healthz` | GET | Liveness/readiness probe |
| `/v1/models` | GET | List models from upstream |
| `/v1/chat/completions` | POST | Chat completions proxy |
| `/v1/messages` | POST | Anthropic-style messages proxy |
| `/v1/messages/count_tokens` | POST | Token counting proxy |
| `/v1/embeddings` | POST | Embeddings proxy |
| `/v1/responses` | POST | Responses API create |
| `/v1/responses/{id}` | GET, DELETE | Retrieve or delete stored response |
| `/v1/responses/{id}/cancel` | POST | Cancel background response |
| `/v1/responses/{id}/input_items` | GET | List input items for stored response |
| `/v1/responses/input_tokens` | POST | Input token counting |

## Configuration

| Variable | Default | Role |
| --- | --- | --- |
| `BIND_ADDR` | `0.0.0.0:8080` | HTTP listen address |
| `UPSTREAM_BASE_URL` | `http://vllm:8000/v1` | Default upstream OpenAI-compatible base URL |
| `DEFAULT_MODEL` | `google/gemma-4-31B-it` | Default model when requests omit `model` |
| `UPSTREAM_API_KEY` | (none) | Optional bearer token forwarded to upstream |
| `MODEL_UPSTREAMS` | (empty) | Comma-separated `model=url` overrides |
| `RESPONSES_API_STORE_ENABLED` | `false` | Enable Responses API persistence |
| `RESPONSES_API_STORE_ENDPOINT` | (required when enabled) | gRPC store endpoint (`http://host:port`) |
| `RESPONSE_ID_STORE_TTL_SECONDS` | `86400` | Stored response TTL |
| `RESPONSES_BACKGROUND_ENABLED` | `false` | Enqueue `background=true` jobs to the store |
| `BACKGROUND_QUEUE_CONSUMER_GROUP` | `duihua-background` | Consumer group for background queue |
| `RUST_LOG` | `info` | Tracing filter |

## Local development

### Rust (native)

```bash
cargo build
BIND_ADDR=127.0.0.1:8080 \
UPSTREAM_BASE_URL=http://127.0.0.1:8000/v1 \
RESPONSES_API_STORE_ENABLED=false \
cargo run
```

### Docker Compose

Gateway only (point `UPSTREAM_BASE_URL` at a host-local inference server):

```bash
UPSTREAM_BASE_URL=http://host.docker.internal:8000/v1 docker compose up --build
```

Full local stack with bundled inference and Responses API store:

```bash
RESPONSES_API_STORE_ENABLED=true docker compose --profile inference --profile store up --build
```

Run the full local validation suite:

```bash
make ci
```

Build a local container image:

```bash
make docker
```

## Helm deployment

Install the chart directly:

```bash
helm upgrade --install duihua-gateway charts/duihua-gateway \
  --namespace duihua \
  --create-namespace
```

With Responses API store persistence:

```yaml
responsesApiStore:
  enabled: true
  endpoint: http://responses-api-store:50051
  backgroundJobs:
    enabled: true
    consumerGroup: duihua-background
```

When embedded in [duihua-ai-services](https://github.com/beranekio/duihua-ai-services), add an OCI chart dependency (same pattern as `responses-api-store`):

```yaml
dependencies:
  - name: duihua-gateway
    version: 0.0.0-<git-sha>
    repository: oci://ghcr.io/beranekio/charts
```

Parent chart values use the `duihua-gateway:` key (replacing the former inline `gateway:` templates).

## CI and releases

- **Validate** (PRs and `main`): Rust fmt/clippy/tests, Helm lint, Dockerfile lint, Docker build, HTTP smoke test, kind Helm smoke test.
- **Publish** (after Validate succeeds on `main` push): pushes `ghcr.io/beranekio/duihua-gateway:<git-sha>` and publishes the Helm chart to `oci://ghcr.io/beranekio/charts` as `0.0.0-<git-sha>`.

## Integration with duihua-ai-services

The gateway is intended to replace the inline `gateway` templates in `charts/duihua-ai-services`. Parent charts should:

- Depend on this repo's published OCI Helm chart.
- Set `env.modelUpstreams` from inference proxy Services when using per-model routing.
- Wire `responsesApiStore.endpoint` to the `responses-api-store` subchart Service when both are enabled.
- Keep ingress and background-worker configuration in the parent chart.