use crate::model::{
    AppSnapshot, ChatMessage, ObserverFrame, ObserverFrameInput, Presence, ServerEvent, TaskItem,
    TaskStatus, TimelineEvent, now_unix_ms,
};
use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};
use tokio::sync::broadcast;

const MAX_CHAT_MESSAGES: usize = 500;
const MAX_TASKS: usize = 500;
const MAX_TIMELINE_EVENTS: usize = 1000;
const MAX_OBSERVER_FRAMES: usize = 500;

#[derive(Debug, Default)]
struct AppStateData {
    next_chat_id: u64,
    next_task_id: u64,
    next_timeline_id: u64,
    chat: Vec<ChatMessage>,
    tasks: Vec<TaskItem>,
    timeline: Vec<TimelineEvent>,
    presence: BTreeMap<String, Presence>,
    observer_frames: Vec<ObserverFrame>,
}

impl AppStateData {
    fn next_chat_id(&mut self) -> u64 {
        self.next_chat_id = self.next_chat_id.saturating_add(1);
        self.next_chat_id
    }

    fn next_task_id(&mut self) -> u64 {
        self.next_task_id = self.next_task_id.saturating_add(1);
        self.next_task_id
    }

    fn next_timeline_id(&mut self) -> u64 {
        self.next_timeline_id = self.next_timeline_id.saturating_add(1);
        self.next_timeline_id
    }

    fn add_timeline_event(&mut self, kind: String, text: String) -> TimelineEvent {
        let event = TimelineEvent {
            id: self.next_timeline_id(),
            kind,
            text,
            created_at_ms: now_unix_ms(),
        };
        self.timeline.push(event.clone());
        trim_oldest(&mut self.timeline, MAX_TIMELINE_EVENTS);
        event
    }
}

#[derive(Clone)]
pub struct SharedState {
    inner: Arc<RwLock<AppStateData>>,
    tx: broadcast::Sender<ServerEvent>,
}

impl SharedState {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(1024);
        Self {
            inner: Arc::new(RwLock::new(AppStateData::default())),
            tx,
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<ServerEvent> {
        self.tx.subscribe()
    }

    pub fn snapshot(&self) -> AppSnapshot {
        let inner = self.inner.read().expect("state lock poisoned");
        AppSnapshot {
            chat: inner.chat.clone(),
            tasks: inner.tasks.clone(),
            timeline: inner.timeline.clone(),
            presence: inner.presence.values().cloned().collect(),
            observer_frames: inner.observer_frames.clone(),
            generated_at_ms: now_unix_ms(),
        }
    }

    pub fn list_chat(&self) -> Vec<ChatMessage> {
        let inner = self.inner.read().expect("state lock poisoned");
        inner.chat.clone()
    }

    pub fn list_tasks(&self) -> Vec<TaskItem> {
        let inner = self.inner.read().expect("state lock poisoned");
        inner.tasks.clone()
    }

    pub fn list_timeline(&self) -> Vec<TimelineEvent> {
        let inner = self.inner.read().expect("state lock poisoned");
        inner.timeline.clone()
    }

    pub fn list_presence(&self) -> Vec<Presence> {
        let inner = self.inner.read().expect("state lock poisoned");
        inner.presence.values().cloned().collect()
    }

    pub fn list_observer_frames(&self) -> Vec<ObserverFrame> {
        let inner = self.inner.read().expect("state lock poisoned");
        inner.observer_frames.clone()
    }

    pub fn create_chat(&self, user: String, text: String) -> ChatMessage {
        let (message, timeline_event) = {
            let mut inner = self.inner.write().expect("state lock poisoned");
            let message = ChatMessage {
                id: inner.next_chat_id(),
                user: user.clone(),
                text: text.clone(),
                created_at_ms: now_unix_ms(),
            };
            inner.chat.push(message.clone());
            trim_oldest(&mut inner.chat, MAX_CHAT_MESSAGES);

            let timeline_event =
                inner.add_timeline_event("chat".to_string(), format!("{user}: {text}"));
            (message, timeline_event)
        };

        self.publish(ServerEvent::ChatCreated(message.clone()));
        self.publish(ServerEvent::TimelineCreated(timeline_event));
        message
    }

    pub fn create_task(&self, title: String, assignee: Option<String>) -> TaskItem {
        let (task, timeline_event) = {
            let mut inner = self.inner.write().expect("state lock poisoned");
            let now = now_unix_ms();
            let task = TaskItem {
                id: inner.next_task_id(),
                title: title.clone(),
                assignee,
                status: TaskStatus::Todo,
                created_at_ms: now,
                updated_at_ms: now,
            };
            inner.tasks.push(task.clone());
            trim_oldest(&mut inner.tasks, MAX_TASKS);

            let timeline_event = inner.add_timeline_event(
                "task".to_string(),
                format!("Task #{id} created: {title}", id = task.id),
            );
            (task, timeline_event)
        };

        self.publish(ServerEvent::TaskCreated(task.clone()));
        self.publish(ServerEvent::TimelineCreated(timeline_event));
        task
    }

    pub fn update_task_status(&self, id: u64, status: TaskStatus) -> Option<TaskItem> {
        let (task, timeline_event) = {
            let mut inner = self.inner.write().expect("state lock poisoned");
            let idx = inner.tasks.iter().position(|task| task.id == id)?;
            inner.tasks[idx].status = status;
            inner.tasks[idx].updated_at_ms = now_unix_ms();
            let task = inner.tasks[idx].clone();

            let timeline_event = inner.add_timeline_event(
                "task".to_string(),
                format!("Task #{id} status -> {}", task_status_label(&task.status)),
            );
            (task, timeline_event)
        };

        self.publish(ServerEvent::TaskUpdated(task.clone()));
        self.publish(ServerEvent::TimelineCreated(timeline_event));
        Some(task)
    }

    pub fn create_timeline_event(&self, kind: String, text: String) -> TimelineEvent {
        let event = {
            let mut inner = self.inner.write().expect("state lock poisoned");
            inner.add_timeline_event(kind, text)
        };
        self.publish(ServerEvent::TimelineCreated(event.clone()));
        event
    }

    pub fn upsert_presence(&self, user: String, status: String) -> Presence {
        let (presence, timeline_event) = {
            let mut inner = self.inner.write().expect("state lock poisoned");
            let key = user.to_ascii_lowercase();
            let presence = Presence {
                user: user.clone(),
                status: status.clone(),
                updated_at_ms: now_unix_ms(),
            };
            inner.presence.insert(key, presence.clone());

            let timeline_event =
                inner.add_timeline_event("presence".to_string(), format!("{user} is now {status}"));
            (presence, timeline_event)
        };

        self.publish(ServerEvent::PresenceUpdated(presence.clone()));
        self.publish(ServerEvent::TimelineCreated(timeline_event));
        presence
    }

    pub fn record_observer_frame(&self, input: ObserverFrameInput) -> ObserverFrame {
        let (frame, timeline_event) = {
            let mut inner = self.inner.write().expect("state lock poisoned");
            let frame = ObserverFrame {
                path: input.path,
                filename: input.filename.clone(),
                size_bytes: input.size_bytes,
                modified_at_ms: input.modified_at_ms,
                observed_at_ms: now_unix_ms(),
            };
            inner.observer_frames.push(frame.clone());
            trim_oldest(&mut inner.observer_frames, MAX_OBSERVER_FRAMES);

            let timeline_event = inner.add_timeline_event(
                "observer".to_string(),
                format!("Frame observed: {}", input.filename),
            );
            (frame, timeline_event)
        };

        self.publish(ServerEvent::ObserverFrame(frame.clone()));
        self.publish(ServerEvent::TimelineCreated(timeline_event));
        frame
    }

    pub fn publish_snapshot(&self) {
        self.publish(ServerEvent::Snapshot(self.snapshot()));
    }

    fn publish(&self, event: ServerEvent) {
        let _ = self.tx.send(event);
    }
}

fn trim_oldest<T>(items: &mut Vec<T>, max_items: usize) {
    if items.len() <= max_items {
        return;
    }
    let remove_count = items.len().saturating_sub(max_items);
    items.drain(0..remove_count);
}

fn task_status_label(status: &TaskStatus) -> &'static str {
    match status {
        TaskStatus::Todo => "todo",
        TaskStatus::InProgress => "in_progress",
        TaskStatus::Done => "done",
    }
}
