use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use uuid::Uuid;

use crate::domain::note::{
    CreateNoteError, DeleteNoteError, ListNotesError, Note, NoteRepository, NoteText,
    UpdateNoteError,
};

/// The in-memory map from the axum todos example, behind [`NoteRepository`].
#[derive(Clone, Default)]
pub struct MemoryNotes {
    notes: Arc<RwLock<HashMap<Uuid, Record>>>,
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

    fn read(&self) -> Result<std::sync::RwLockReadGuard<'_, HashMap<Uuid, Record>>, anyhow::Error> {
        self.notes
            .read()
            .map_err(|_| anyhow::anyhow!("note store lock poisoned"))
    }

    fn write(
        &self,
    ) -> Result<std::sync::RwLockWriteGuard<'_, HashMap<Uuid, Record>>, anyhow::Error> {
        self.notes
            .write()
            .map_err(|_| anyhow::anyhow!("note store lock poisoned"))
    }
}

impl NoteRepository for MemoryNotes {
    async fn create(&self, owner: &str, text: &NoteText) -> Result<Note, CreateNoteError> {
        let mut notes = self.write().map_err(CreateNoteError::Unknown)?;
        let id = Uuid::new_v4();
        notes.insert(
            id,
            Record {
                owner: owner.to_string(),
                text: text.clone(),
                completed: false,
            },
        );
        Ok(Note::from_stored(id, text.clone(), false))
    }

    async fn list(&self, owner: &str) -> Result<Vec<Note>, ListNotesError> {
        let notes = self.read().map_err(ListNotesError::Unknown)?;
        Ok(notes
            .iter()
            .filter(|(_, record)| record.owner == owner)
            .map(|(id, record)| Note::from_stored(*id, record.text.clone(), record.completed))
            .collect())
    }

    async fn update(
        &self,
        owner: &str,
        id: Uuid,
        text: Option<&NoteText>,
        completed: Option<bool>,
    ) -> Result<Note, UpdateNoteError> {
        let mut notes = self.write().map_err(UpdateNoteError::Unknown)?;
        let Some(record) = notes.get_mut(&id).filter(|record| record.owner == owner) else {
            return Err(UpdateNoteError::NotFound);
        };
        if let Some(text) = text {
            record.text = text.clone();
        }
        if let Some(completed) = completed {
            record.completed = completed;
        }
        Ok(Note::from_stored(id, record.text.clone(), record.completed))
    }

    async fn delete(&self, owner: &str, id: Uuid) -> Result<(), DeleteNoteError> {
        let mut notes = self.write().map_err(DeleteNoteError::Unknown)?;
        match notes.get(&id) {
            Some(record) if record.owner == owner => {
                notes.remove(&id);
                Ok(())
            }
            _ => Err(DeleteNoteError::NotFound),
        }
    }
}
