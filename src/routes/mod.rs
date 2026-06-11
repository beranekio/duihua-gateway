mod chat;
mod embeddings;
mod health;
mod messages;
mod models;
mod responses;

use std::sync::Arc;

use axum::{
    extract::DefaultBodyLimit,
    routing::{get, post},
    Router,
};
use tower_http::{cors::CorsLayer, trace::TraceLayer};

use crate::state::AppState;

/// Axum's default JSON body limit is 2 MiB, which rejects common multimodal payloads.
const MAX_REQUEST_BODY_BYTES: usize = 64 * 1024 * 1024;

pub fn router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/healthz", get(health::healthz))
        .route("/v1/models", get(models::list_models))
        .route("/v1/chat/completions", post(chat::chat_completions))
        .route("/v1/messages", post(messages::messages))
        .route(
            "/v1/messages/count_tokens",
            post(messages::messages_count_tokens),
        )
        .route("/v1/responses", post(responses::responses))
        .route(
            "/v1/responses/input_tokens",
            post(responses::response_input_tokens),
        )
        .route("/v1/embeddings", post(embeddings::embeddings))
        .route(
            "/v1/responses/{response_id}",
            get(responses::get_response).delete(responses::delete_response),
        )
        .route(
            "/v1/responses/{response_id}/cancel",
            post(responses::cancel_response),
        )
        .route(
            "/v1/responses/{response_id}/input_items",
            get(responses::list_response_input_items),
        )
        .layer(DefaultBodyLimit::max(MAX_REQUEST_BODY_BYTES))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
