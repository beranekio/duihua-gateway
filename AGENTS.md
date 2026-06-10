# AGENTS.md

Guidance for human and AI contributors working in this repository.

## Project overview

This repo provides an **OpenAI-compatible HTTP gateway** (Rust, Axum) that proxies inference requests to upstream model runtimes and optionally persists Responses API state via the [responses-api-store](https://github.com/beranekio/responses-api-store) gRPC client.

It was extracted from [beranekio/duihua-ai-services](https://github.com/beranekio/duihua-ai-services) for independent development and release.

## Repository layout

| Path | Purpose |
| --- | --- |
| `src/` | Gateway service source |
| `charts/duihua-gateway/` | Helm subchart |
| `scripts/helm-smoke-kind.sh` | kind-based Helm smoke test |
| `Dockerfile` | Multi-stage build: `rust:1-bookworm` builder, `gcr.io/distroless/cc-debian12:nonroot` runtime |

## Recommended workflow

1. Read `README.md` and the relevant source module before editing.
2. Keep changes focused and minimal to the requested task.
3. Update `README.md` and Helm values/templates when behavior or configuration changes.
4. Run targeted validation for the areas you modified (see [Validation commands](#validation-commands)).

## Validation commands

Run checks that match the files you changed. From the repository root:

### Full CI parity

```bash
make ci
```

### Rust

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test
```

### Helm chart

```bash
helm lint charts/duihua-gateway
helm template duihua-gateway charts/duihua-gateway --debug >/tmp/duihua-gateway-rendered.yaml
```

### Docker

```bash
docker build -t duihua-gateway:local .
```

The builder stage requires `protobuf-compiler` (for the `responses-api-store-client` git dependency). The runtime image is distroless; do not add a shell or package manager to the runtime stage.

### Integration smoke test

```bash
BIND_ADDR=127.0.0.1:18080 RESPONSES_API_STORE_ENABLED=false cargo run &
curl -fsS http://127.0.0.1:18080/healthz
```

### Helm chart smoke test (kind)

```bash
make helm-smoke
```

Requires `kind`, `kubectl`, `helm`, `docker`, and `curl`. CI runs this in the `helm-smoke` job after `docker-build`.

## Editing conventions

- Preserve existing naming and style; match patterns from `duihua-ai-services` where domains overlap.
- Avoid unrelated refactors in the same commit.
- Keep Kubernetes defaults cloud-provider-neutral unless explicitly required.
- Document user-visible changes in `README.md`.

## Integration with duihua-ai-services

When wiring this service back into `duihua-ai-services`:

- Replace inline `gateway-deployment.yaml` / `gateway-service.yaml` templates with an OCI subchart dependency on `duihua-gateway`.
- Parent chart should set `env.modelUpstreams` from inference proxy Services.
- Wire `responsesApiStore.endpoint` to the `responses-api-store` subchart Service when both are enabled.
- Ingress and background-worker remain in the parent chart.

## Agent-specific notes

### Opening pull requests

When creating a PR, **add a GitHub label that identifies the agent** (or tooling) that authored it.

| Agent / tool | Label |
| --- | --- |
| ChatGPT Codex | `codex` |
| Cursor | `cursor` |
| Claude | `claude` |
| Grok | `grok` |

```bash
gh pr create --label grok ...
```

Include in the PR description:

- What changed and why
- How it was validated (exact commands)
- Whether Helm changes affect downstream consumers in `duihua-ai-services`