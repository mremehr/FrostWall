use crate::model::{
    CreateChatRequest, CreateTaskRequest, CreateTimelineRequest, HealthResponse, ServerEvent,
    UpdateTaskStatusRequest, UpsertPresenceRequest,
};
use crate::state::SharedState;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, patch};
use axum::{Json, Router};
use futures_util::stream::SplitSink;
use futures_util::{SinkExt, StreamExt};
use tower_http::cors::{Any, CorsLayer};
use tracing::warn;

type ApiError = (StatusCode, String);
type ApiResult<T> = Result<Json<T>, ApiError>;

pub fn router(state: SharedState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/api/state", get(get_state))
        .route("/api/chat", get(list_chat).post(post_chat))
        .route("/api/tasks", get(list_tasks).post(post_task))
        .route("/api/tasks/{id}/status", patch(patch_task_status))
        .route("/api/timeline", get(list_timeline).post(post_timeline))
        .route("/api/presence", get(list_presence).post(post_presence))
        .route("/api/observer/frames", get(list_observer_frames))
        .route("/ws", get(ws_upgrade))
        .with_state(state)
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        )
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse { ok: true })
}

async fn get_state(State(state): State<SharedState>) -> Json<crate::model::AppSnapshot> {
    Json(state.snapshot())
}

async fn list_chat(State(state): State<SharedState>) -> Json<Vec<crate::model::ChatMessage>> {
    Json(state.list_chat())
}

async fn post_chat(
    State(state): State<SharedState>,
    Json(body): Json<CreateChatRequest>,
) -> ApiResult<crate::model::ChatMessage> {
    let user = non_empty(&body.user, "user")?;
    let text = non_empty(&body.text, "text")?;
    Ok(Json(state.create_chat(user, text)))
}

async fn list_tasks(State(state): State<SharedState>) -> Json<Vec<crate::model::TaskItem>> {
    Json(state.list_tasks())
}

async fn post_task(
    State(state): State<SharedState>,
    Json(body): Json<CreateTaskRequest>,
) -> ApiResult<crate::model::TaskItem> {
    let title = non_empty(&body.title, "title")?;
    let assignee = body.assignee.map(|value| value.trim().to_string());
    Ok(Json(
        state.create_task(title, assignee.filter(|v| !v.is_empty())),
    ))
}

async fn patch_task_status(
    Path(id): Path<u64>,
    State(state): State<SharedState>,
    Json(body): Json<UpdateTaskStatusRequest>,
) -> ApiResult<crate::model::TaskItem> {
    match state.update_task_status(id, body.status) {
        Some(task) => Ok(Json(task)),
        None => Err((StatusCode::NOT_FOUND, format!("task {id} not found"))),
    }
}

async fn list_timeline(State(state): State<SharedState>) -> Json<Vec<crate::model::TimelineEvent>> {
    Json(state.list_timeline())
}

async fn post_timeline(
    State(state): State<SharedState>,
    Json(body): Json<CreateTimelineRequest>,
) -> ApiResult<crate::model::TimelineEvent> {
    let kind = non_empty(&body.kind, "kind")?;
    let text = non_empty(&body.text, "text")?;
    Ok(Json(state.create_timeline_event(kind, text)))
}

async fn list_presence(State(state): State<SharedState>) -> Json<Vec<crate::model::Presence>> {
    Json(state.list_presence())
}

async fn post_presence(
    State(state): State<SharedState>,
    Json(body): Json<UpsertPresenceRequest>,
) -> ApiResult<crate::model::Presence> {
    let user = non_empty(&body.user, "user")?;
    let status = non_empty(&body.status, "status")?;
    Ok(Json(state.upsert_presence(user, status)))
}

async fn list_observer_frames(
    State(state): State<SharedState>,
) -> Json<Vec<crate::model::ObserverFrame>> {
    Json(state.list_observer_frames())
}

async fn ws_upgrade(ws: WebSocketUpgrade, State(state): State<SharedState>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| ws_client(socket, state))
}

async fn ws_client(socket: WebSocket, state: SharedState) {
    let (mut sender, mut receiver) = socket.split();
    if send_event(&mut sender, &ServerEvent::Snapshot(state.snapshot()))
        .await
        .is_err()
    {
        return;
    }

    let mut rx = state.subscribe();
    loop {
        tokio::select! {
            incoming = receiver.next() => {
                match incoming {
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Ok(Message::Ping(payload))) => {
                        if sender.send(Message::Pong(payload)).await.is_err() {
                            break;
                        }
                    }
                    Some(Ok(_)) => {}
                    Some(Err(err)) => {
                        warn!("websocket receive error: {err}");
                        break;
                    }
                }
            }
            outgoing = rx.recv() => {
                match outgoing {
                    Ok(event) => {
                        if send_event(&mut sender, &event).await.is_err() {
                            break;
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
                        warn!("websocket lagged by {skipped} events, resyncing snapshot");
                        if send_event(&mut sender, &ServerEvent::Snapshot(state.snapshot())).await.is_err() {
                            break;
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                }
            }
        }
    }
}

async fn send_event(
    sender: &mut SplitSink<WebSocket, Message>,
    event: &ServerEvent,
) -> anyhow::Result<()> {
    let payload = serde_json::to_string(event)?;
    sender.send(Message::Text(payload.into())).await?;
    Ok(())
}

fn non_empty(value: &str, field_name: &str) -> Result<String, ApiError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            format!("{field_name} must not be empty"),
        ));
    }
    Ok(trimmed.to_string())
}
