use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode, Uri},
    response::{IntoResponse, Response},
    Json,
};
use serde_json::{json, Value};
use tracing::error;

use crate::{
    background,
    error::{previous_response_not_ready, ErrorBody, ErrorResponse},
    models::{
        continuation_input, disable_upstream_response_store, is_background_request,
        normalized_input, request_input, response_model, set_request_input,
        should_persist_gateway_response, should_store_response, ResponsesRequest,
    },
    state::{upstream_for_model, AppState},
    store::load_response,
    upstream::{proxy_request, proxy_response_request, resolve_upstream_authorization},
};

fn continuation_upstream(
    state: &AppState,
    previous_upstream: &str,
    explicit_model: Option<&str>,
    resolved_model: &str,
) -> String {
    if explicit_model.is_some() {
        upstream_for_model(state, resolved_model).to_string()
    } else {
        previous_upstream.to_string()
    }
}

pub async fn responses(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(mut payload): Json<ResponsesRequest>,
) -> Response {
    let background = is_background_request(&payload);
    if background {
        if !state.responses_api_store_enabled {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: ErrorBody {
                        message: "Background responses require the gateway-owned response store."
                            .to_string(),
                        error_type: "invalid_request_error",
                        param: "background",
                        code: 503,
                    },
                }),
            )
                .into_response();
        }
        if !should_store_response(&payload) {
            return (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: ErrorBody {
                        message: "Background responses require store=true.".to_string(),
                        error_type: "invalid_request_error",
                        param: "store",
                        code: 400,
                    },
                }),
            )
                .into_response();
        }
        if payload.extra.get("stream").and_then(Value::as_bool) == Some(true) {
            return (
                StatusCode::NOT_IMPLEMENTED,
                "streaming background responses are not supported",
            )
                .into_response();
        }
    }

    let (upstream, input) = if let Some(previous_response_id) = payload.previous_response_id.take()
    {
        let previous = match load_response(state.as_ref(), &previous_response_id).await {
            Ok(previous) => previous,
            Err(response) => return response,
        };
        if background::is_in_flight_background(&previous) {
            return previous_response_not_ready();
        }
        let explicit_model = payload.model.clone();
        if payload.model.is_none() {
            payload.model =
                response_model(&previous.response).or_else(|| Some(state.default_model.clone()));
        }
        let resolved_model = payload
            .model
            .as_deref()
            .unwrap_or(state.default_model.as_str());
        let upstream = continuation_upstream(
            state.as_ref(),
            &previous.upstream,
            explicit_model.as_deref(),
            resolved_model,
        );
        let input = continuation_input(&previous, request_input(&payload));
        set_request_input(&mut payload, input.clone());
        (upstream, input)
    } else {
        if payload.model.is_none() {
            payload.model = Some(state.default_model.clone());
        }
        let selected_model = payload
            .model
            .as_deref()
            .unwrap_or(state.default_model.as_str())
            .to_string();
        (
            upstream_for_model(state.as_ref(), &selected_model).to_string(),
            normalized_input(request_input(&payload)),
        )
    };

    let persist_response =
        should_persist_gateway_response(state.responses_api_store_enabled, &payload);

    if state.responses_api_store_enabled {
        disable_upstream_response_store(&mut payload);
    }

    if background {
        return create_background_response(state, headers, payload, upstream, input).await;
    }

    proxy_response_request(state, headers, payload, upstream, input, persist_response).await
}

async fn create_background_response(
    state: Arc<AppState>,
    headers: HeaderMap,
    payload: ResponsesRequest,
    upstream: String,
    input: Vec<Value>,
) -> Response {
    let upstream_authorization = resolve_upstream_authorization(&headers, state.as_ref());
    let model = payload
        .model
        .clone()
        .unwrap_or_else(|| state.default_model.clone());
    let response_id = background::generate_response_id();
    let request_value = serde_json::to_value(&payload).unwrap_or(Value::Null);
    let upstream_request = background::build_upstream_request(&request_value);
    let queued_response = background::build_queued_response(&response_id, &model, &request_value);

    match background::enqueue_background_response(
        state.as_ref(),
        response_id,
        upstream,
        input,
        upstream_request,
        queued_response.clone(),
        upstream_authorization,
    )
    .await
    {
        Ok(()) => (StatusCode::OK, Json(queued_response)).into_response(),
        Err(response) => response,
    }
}

pub async fn response_input_tokens(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(mut payload): Json<ResponsesRequest>,
) -> Response {
    let upstream = if let Some(previous_response_id) = payload.previous_response_id.take() {
        let previous = match load_response(state.as_ref(), &previous_response_id).await {
            Ok(previous) => previous,
            Err(response) => return response,
        };
        if background::is_in_flight_background(&previous) {
            return previous_response_not_ready();
        }
        let explicit_model = payload.model.clone();
        if payload.model.is_none() {
            payload.model =
                response_model(&previous.response).or_else(|| Some(state.default_model.clone()));
        }
        let resolved_model = payload
            .model
            .as_deref()
            .unwrap_or(state.default_model.as_str());
        let upstream = continuation_upstream(
            state.as_ref(),
            &previous.upstream,
            explicit_model.as_deref(),
            resolved_model,
        );
        let input = continuation_input(&previous, request_input(&payload));
        set_request_input(&mut payload, input);
        upstream
    } else {
        if payload.model.is_none() {
            payload.model = Some(state.default_model.clone());
        }
        let selected_model = payload
            .model
            .as_deref()
            .unwrap_or(state.default_model.as_str())
            .to_string();
        upstream_for_model(state.as_ref(), &selected_model).to_string()
    };

    proxy_request(
        state.as_ref(),
        headers,
        payload,
        &upstream,
        "responses/input_tokens",
    )
    .await
}

pub async fn get_response(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    uri: Uri,
    Path(response_id): Path<String>,
) -> Response {
    let _ = (headers, uri);
    match load_response(state.as_ref(), &response_id).await {
        Ok(stored) => Json(stored.response).into_response(),
        Err(response) => response,
    }
}

pub async fn delete_response(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(response_id): Path<String>,
) -> Response {
    let _ = headers;
    let stored = match crate::store::load_stored_response(state.as_ref(), &response_id).await {
        Ok(stored) => stored,
        Err(response) => return response,
    };
    if let Err(response) =
        background::finalize_background_deletion(state.as_ref(), &response_id, &stored).await
    {
        return response;
    }
    Json(json!({"id": response_id, "object": "response.deleted", "deleted": true})).into_response()
}

pub async fn cancel_response(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(response_id): Path<String>,
) -> Response {
    let _ = headers;
    let mut stored = match crate::store::load_stored_response(state.as_ref(), &response_id).await {
        Ok(stored) => stored,
        Err(response) => return response,
    };
    if stored.response.get("background").and_then(Value::as_bool) != Some(true) {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": {"message": "Cannot cancel a synchronous response.", "type": "invalid_request_error", "param": "response_id", "code": 400}})),
        )
            .into_response();
    }

    let status = background::stored_response_status(&stored);
    if matches!(
        status,
        Some("completed" | "failed" | "cancelled" | "incomplete")
    ) {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": {"message": format!("Cannot cancel a response that is already {}.", status.unwrap_or("unknown")), "type": "invalid_request_error", "param": "response_id", "code": 400}})),
        )
            .into_response();
    }

    let cancelled = background::build_cancelled_response(&stored, &response_id);
    stored.response = cancelled.clone();
    stored.pending_upstream_request = None;
    stored.upstream_authorization = None;
    if let Some(response_store) = &state.response_store {
        if let Err(e) = response_store.store(&response_id, &stored).await {
            error!("failed to persist cancelled background response {response_id}: {e}");
            return (StatusCode::BAD_GATEWAY, "response id store write failed").into_response();
        }
    }

    Json(cancelled).into_response()
}

pub async fn list_response_input_items(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    uri: Uri,
    Path(response_id): Path<String>,
) -> Response {
    let _ = (headers, uri);
    match load_response(state.as_ref(), &response_id).await {
        Ok(stored) => {
            Json(json!({"object": "list", "data": stored.input, "has_more": false})).into_response()
        }
        Err(response) => response,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    use reqwest::Client;

    fn test_state() -> AppState {
        AppState {
            upstream_base: "http://default:8000/v1".to_string(),
            model_upstreams: HashMap::from([(
                "model-b".to_string(),
                "http://model-b:8000/v1".to_string(),
            )]),
            default_model: "model-default".to_string(),
            upstream_api_key: None,
            client: Client::new(),
            responses_api_store_enabled: false,
            background_jobs_enabled: false,
            response_store: None,
        }
    }

    #[test]
    fn continuation_upstream_uses_previous_upstream_without_explicit_model() {
        let state = test_state();
        assert_eq!(
            continuation_upstream(&state, "http://model-a:8000/v1", None, "model-b"),
            "http://model-a:8000/v1"
        );
    }

    #[test]
    fn continuation_upstream_routes_explicit_model_override() {
        let state = test_state();
        assert_eq!(
            continuation_upstream(&state, "http://model-a:8000/v1", Some("model-b"), "model-b"),
            "http://model-b:8000/v1"
        );
    }
}
