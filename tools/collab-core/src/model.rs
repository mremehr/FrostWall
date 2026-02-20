use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

pub fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub id: u64,
    pub user: String,
    pub text: String,
    pub created_at_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    #[default]
    Todo,
    InProgress,
    Done,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskItem {
    pub id: u64,
    pub title: String,
    pub assignee: Option<String>,
    pub status: TaskStatus,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineEvent {
    pub id: u64,
    pub kind: String,
    pub text: String,
    pub created_at_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Presence {
    pub user: String,
    pub status: String,
    pub updated_at_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObserverFrame {
    pub path: String,
    pub filename: String,
    pub size_bytes: u64,
    pub modified_at_ms: u64,
    pub observed_at_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct AppSnapshot {
    pub chat: Vec<ChatMessage>,
    pub tasks: Vec<TaskItem>,
    pub timeline: Vec<TimelineEvent>,
    pub presence: Vec<Presence>,
    pub observer_frames: Vec<ObserverFrame>,
    pub generated_at_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct HealthResponse {
    pub ok: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateChatRequest {
    pub user: String,
    pub text: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateTaskRequest {
    pub title: String,
    pub assignee: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateTaskStatusRequest {
    pub status: TaskStatus,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateTimelineRequest {
    pub kind: String,
    pub text: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpsertPresenceRequest {
    pub user: String,
    pub status: String,
}

#[derive(Debug, Clone)]
pub struct ObserverFrameInput {
    pub path: String,
    pub filename: String,
    pub size_bytes: u64,
    pub modified_at_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", content = "data")]
pub enum ServerEvent {
    #[serde(rename = "snapshot")]
    Snapshot(AppSnapshot),
    #[serde(rename = "chat.created")]
    ChatCreated(ChatMessage),
    #[serde(rename = "task.created")]
    TaskCreated(TaskItem),
    #[serde(rename = "task.updated")]
    TaskUpdated(TaskItem),
    #[serde(rename = "timeline.created")]
    TimelineCreated(TimelineEvent),
    #[serde(rename = "presence.updated")]
    PresenceUpdated(Presence),
    #[serde(rename = "observer.frame")]
    ObserverFrame(ObserverFrame),
}
