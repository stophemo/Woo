use once_cell::sync::Lazy;
use std::sync::Mutex;

#[derive(Debug, Clone, Default)]
pub struct EditorPresence {
    pub document_id: Option<String>,
    pub dirty: bool,
}

static PRESENCE: Lazy<Mutex<EditorPresence>> =
    Lazy::new(|| Mutex::new(EditorPresence::default()));

fn lock() -> std::sync::MutexGuard<'static, EditorPresence> {
    PRESENCE.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

pub fn set(document_id: Option<String>, dirty: bool) {
    let mut presence = lock();
    presence.document_id = document_id;
    presence.dirty = dirty;
}

pub fn clear() {
    set(None, false);
}

pub fn is_editing(document_id: &str) -> bool {
    let presence = lock();
    presence.document_id.as_deref() == Some(document_id) && presence.dirty
}
