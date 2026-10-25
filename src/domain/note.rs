use std::future::Future;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Error as AnyhowError;
use thiserror::Error;
use uuid::Uuid;

use super::user::UserId;

const MAX_BODY_CHARS: usize = 4_000;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NoteId(Uuid);

impl NoteId {
    pub fn parse(raw: &str) -> Result<Self, NoteIdError> {
        Uuid::parse_str(raw).map(Self).map_err(|_| NoteIdError)
    }

    pub fn as_str(&self) -> String {
        self.0.to_string()
    }
}

impl std::fmt::Display for NoteId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Error)]
#[error("note id is not a uuid")]
pub struct NoteIdError;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NoteBody(String);

#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum NoteBodyError {
    #[error("note body cannot be empty")]
    Empty,
    #[error("note body is longer than {MAX_BODY_CHARS} characters")]
    TooLong,
}

impl NoteBody {
    pub fn new(raw: &str) -> Result<Self, NoteBodyError> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(NoteBodyError::Empty);
        }
        if trimmed.chars().count() > MAX_BODY_CHARS {
            return Err(NoteBodyError::TooLong);
        }
        Ok(Self(trimmed.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Note {
    id: NoteId,
    owner: UserId,
    body: NoteBody,
    created_at_ms: i64,
}

impl Note {
    pub fn create(owner: UserId, body: NoteBody) -> Self {
        Self {
            id: NoteId(Uuid::new_v4()),
            owner,
            body,
            created_at_ms: now_ms(),
        }
    }

    pub fn from_stored(id: NoteId, owner: UserId, body: NoteBody, created_at_ms: i64) -> Self {
        Self {
            id,
            owner,
            body,
            created_at_ms,
        }
    }

    pub fn id(&self) -> &NoteId {
        &self.id
    }

    pub fn owner(&self) -> &UserId {
        &self.owner
    }

    pub fn body(&self) -> &NoteBody {
        &self.body
    }

    pub fn created_at_ms(&self) -> i64 {
        self.created_at_ms
    }
}

#[derive(Debug, Error)]
pub enum CreateNoteError {
    #[error("note store failed")]
    Unknown(#[source] AnyhowError),
}

#[derive(Debug, Error)]
pub enum ListNotesError {
    #[error("note store failed")]
    Unknown(#[source] AnyhowError),
}

pub trait NoteRepository: Clone + Send + Sync + 'static {
    fn insert(&self, note: &Note) -> impl Future<Output = Result<(), CreateNoteError>> + Send;
    fn list_by_owner(
        &self,
        owner: &UserId,
    ) -> impl Future<Output = Result<Vec<Note>, ListNotesError>> + Send;
}

pub trait NoteService: Clone + Send + Sync + 'static {
    fn create(
        &self,
        owner: &UserId,
        body: &NoteBody,
    ) -> impl Future<Output = Result<Note, CreateNoteError>> + Send;
    fn list(
        &self,
        owner: &UserId,
    ) -> impl Future<Output = Result<Vec<Note>, ListNotesError>> + Send;
}

#[derive(Debug, Clone)]
pub struct Service<R> {
    repo: R,
}

impl<R> Service<R> {
    pub fn new(repo: R) -> Self {
        Self { repo }
    }
}

impl<R> NoteService for Service<R>
where
    R: NoteRepository,
{
    async fn create(&self, owner: &UserId, body: &NoteBody) -> Result<Note, CreateNoteError> {
        let note = Note::create(owner.clone(), body.clone());
        self.repo.insert(&note).await?;
        Ok(note)
    }

    async fn list(&self, owner: &UserId) -> Result<Vec<Note>, ListNotesError> {
        let mut notes = self.repo.list_by_owner(owner).await?;
        notes.sort_by(|left, right| {
            left.created_at_ms()
                .cmp(&right.created_at_ms())
                .then_with(|| left.id().cmp(right.id()))
        });
        Ok(notes)
    }
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| i64::try_from(duration.as_millis()).unwrap_or(i64::MAX))
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::*;

    #[derive(Clone, Default)]
    struct Mem(Arc<Mutex<Vec<Note>>>);

    impl NoteRepository for Mem {
        async fn insert(&self, note: &Note) -> Result<(), CreateNoteError> {
            self.0.lock().expect("lock").push(note.clone());
            Ok(())
        }

        async fn list_by_owner(&self, owner: &UserId) -> Result<Vec<Note>, ListNotesError> {
            Ok(self
                .0
                .lock()
                .expect("lock")
                .iter()
                .filter(|note| note.owner() == owner)
                .cloned()
                .collect())
        }
    }

    #[test]
    fn note_body_trims_and_rejects_blank() {
        assert_eq!(NoteBody::new("  hello  ").unwrap().as_str(), "hello");
        assert_eq!(NoteBody::new("   ").unwrap_err(), NoteBodyError::Empty);
    }

    #[tokio::test]
    async fn service_keeps_notes_per_owner() {
        let service = Service::new(Mem::default());
        let alice = UserId::new("alice").unwrap();
        let bob = UserId::new("bob").unwrap();
        let hello = NoteBody::new("hello").unwrap();

        service.create(&alice, &hello).await.unwrap();
        let alice_notes = service.list(&alice).await.unwrap();
        let bob_notes = service.list(&bob).await.unwrap();

        assert_eq!(alice_notes.len(), 1);
        assert_eq!(alice_notes[0].body().as_str(), "hello");
        assert!(bob_notes.is_empty());
    }
}
