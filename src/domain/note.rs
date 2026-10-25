use std::future::Future;

use anyhow::Error as AnyhowError;
use uuid::Uuid;

/// Validated note text. Empty input never becomes a [`Note`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NoteText(String);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NoteTextEmptyError;

impl NoteText {
    pub fn new(raw: &str) -> Result<Self, NoteTextEmptyError> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            Err(NoteTextEmptyError)
        } else {
            Ok(Self(trimmed.to_string()))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Note {
    id: Uuid,
    text: NoteText,
    completed: bool,
}

impl Note {
    pub fn from_stored(id: Uuid, text: NoteText, completed: bool) -> Self {
        Self {
            id,
            text,
            completed,
        }
    }

    pub fn id(&self) -> Uuid {
        self.id
    }

    pub fn text(&self) -> &NoteText {
        &self.text
    }

    pub fn completed(&self) -> bool {
        self.completed
    }
}

#[derive(Debug)]
pub enum CreateNoteError {
    Unknown(AnyhowError),
}

#[derive(Debug)]
pub enum ListNotesError {
    Unknown(AnyhowError),
}

#[derive(Debug)]
pub enum UpdateNoteError {
    NotFound,
    Unknown(AnyhowError),
}

#[derive(Debug)]
pub enum DeleteNoteError {
    NotFound,
    Unknown(AnyhowError),
}

pub trait NoteRepository: Clone + Send + Sync + 'static {
    fn create(
        &self,
        owner: &str,
        text: &NoteText,
    ) -> impl Future<Output = Result<Note, CreateNoteError>> + Send;

    fn list(&self, owner: &str) -> impl Future<Output = Result<Vec<Note>, ListNotesError>> + Send;

    fn update(
        &self,
        owner: &str,
        id: Uuid,
        text: Option<&NoteText>,
        completed: Option<bool>,
    ) -> impl Future<Output = Result<Note, UpdateNoteError>> + Send;

    fn delete(
        &self,
        owner: &str,
        id: Uuid,
    ) -> impl Future<Output = Result<(), DeleteNoteError>> + Send;
}

pub trait NoteService: Clone + Send + Sync + 'static {
    fn create(
        &self,
        owner: &str,
        text: &NoteText,
    ) -> impl Future<Output = Result<Note, CreateNoteError>> + Send;

    fn list(&self, owner: &str) -> impl Future<Output = Result<Vec<Note>, ListNotesError>> + Send;

    fn update(
        &self,
        owner: &str,
        id: Uuid,
        text: Option<&NoteText>,
        completed: Option<bool>,
    ) -> impl Future<Output = Result<Note, UpdateNoteError>> + Send;

    fn delete(
        &self,
        owner: &str,
        id: Uuid,
    ) -> impl Future<Output = Result<(), DeleteNoteError>> + Send;
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
    async fn create(&self, owner: &str, text: &NoteText) -> Result<Note, CreateNoteError> {
        self.repo.create(owner, text).await
    }

    async fn list(&self, owner: &str) -> Result<Vec<Note>, ListNotesError> {
        self.repo.list(owner).await
    }

    async fn update(
        &self,
        owner: &str,
        id: Uuid,
        text: Option<&NoteText>,
        completed: Option<bool>,
    ) -> Result<Note, UpdateNoteError> {
        self.repo.update(owner, id, text, completed).await
    }

    async fn delete(&self, owner: &str, id: Uuid) -> Result<(), DeleteNoteError> {
        self.repo.delete(owner, id).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn note_text_trims_and_rejects_blank() {
        assert_eq!(NoteText::new("  hello  ").unwrap().as_str(), "hello");
        assert!(NoteText::new("   ").is_err());
    }
}
