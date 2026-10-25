use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use crate::domain::note::{Note, NoteId, NoteNotFound, NoteRepository, NoteText};

/// The in-memory map from the axum todos example, behind [`NoteRepository`].
#[derive(Clone, Default)]
pub struct MemoryNotes {
    notes: Arc<RwLock<HashMap<NoteId, Record>>>,
}

#[derive(Clone)]
struct Record {
    owner: String,
    text: NoteText,
    completed: bool,
}

impl MemoryNotes {
    pub fn new() -> Self {
        Self::default()
    }
}

impl NoteRepository for MemoryNotes {
    async fn create(&self, owner: &str, text: &NoteText) -> Note {
        let id = NoteId::generate();
        self.notes.write().unwrap().insert(
            id,
            Record {
                owner: owner.to_string(),
                text: text.clone(),
                completed: false,
            },
        );
        Note::new(id, text.clone(), false)
    }

    async fn list(&self, owner: &str) -> Vec<Note> {
        self.notes
            .read()
            .unwrap()
            .iter()
            .filter(|(_, record)| record.owner == owner)
            .map(|(id, record)| Note::new(*id, record.text.clone(), record.completed))
            .collect()
    }

    async fn update(
        &self,
        owner: &str,
        id: NoteId,
        text: Option<&NoteText>,
        completed: Option<bool>,
    ) -> Result<Note, NoteNotFound> {
        let mut notes = self.notes.write().unwrap();
        let Some(record) = notes.get_mut(&id).filter(|record| record.owner == owner) else {
            return Err(NoteNotFound);
        };
        if let Some(text) = text {
            record.text = text.clone();
        }
        if let Some(completed) = completed {
            record.completed = completed;
        }
        Ok(Note::new(id, record.text.clone(), record.completed))
    }

    async fn delete(&self, owner: &str, id: NoteId) -> Result<(), NoteNotFound> {
        let mut notes = self.notes.write().unwrap();
        match notes.get(&id) {
            Some(record) if record.owner == owner => {
                notes.remove(&id);
                Ok(())
            }
            _ => Err(NoteNotFound),
        }
    }
}
