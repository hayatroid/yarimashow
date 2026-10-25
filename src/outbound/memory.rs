use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use uuid::Uuid;

use crate::domain::auth::{AuthError, SessionRecord, Sessions};
use crate::domain::note::{CreateNoteError, ListNotesError, Note, NoteRepository};
use crate::domain::user::UserId;

#[derive(Clone, Default)]
pub struct MemoryNotes {
    notes: Arc<Mutex<Vec<Note>>>,
}

impl MemoryNotes {
    pub fn new() -> Self {
        Self::default()
    }
}

impl NoteRepository for MemoryNotes {
    async fn insert(&self, note: &Note) -> Result<(), CreateNoteError> {
        let mut notes = self
            .notes
            .lock()
            .map_err(|_| CreateNoteError::Unknown(anyhow::anyhow!("note store lock poisoned")))?;
        notes.push(note.clone());
        Ok(())
    }

    async fn list_by_owner(&self, owner: &UserId) -> Result<Vec<Note>, ListNotesError> {
        let notes = self
            .notes
            .lock()
            .map_err(|_| ListNotesError::Unknown(anyhow::anyhow!("note store lock poisoned")))?;
        Ok(notes
            .iter()
            .filter(|note| note.owner() == owner)
            .cloned()
            .collect())
    }
}

#[derive(Clone, Default)]
pub struct MemorySessions {
    sessions: Arc<Mutex<HashMap<String, SessionRecord>>>,
}

impl MemorySessions {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Sessions for MemorySessions {
    async fn create(&self, record: SessionRecord) -> Result<String, AuthError> {
        let mut sessions = self
            .sessions
            .lock()
            .map_err(|_| AuthError::Unknown(anyhow::anyhow!("session store lock poisoned")))?;
        let id = Uuid::new_v4().to_string();
        sessions.insert(id.clone(), record);
        Ok(id)
    }

    async fn load(&self, session_id: &str) -> Result<Option<SessionRecord>, AuthError> {
        let sessions = self
            .sessions
            .lock()
            .map_err(|_| AuthError::Unknown(anyhow::anyhow!("session store lock poisoned")))?;
        Ok(sessions.get(session_id).cloned())
    }

    async fn delete(&self, session_id: &str) -> Result<(), AuthError> {
        let mut sessions = self
            .sessions
            .lock()
            .map_err(|_| AuthError::Unknown(anyhow::anyhow!("session store lock poisoned")))?;
        sessions.remove(session_id);
        Ok(())
    }
}
