use std::future::Future;

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

/// Identity of one note. Distinct from any other `Uuid` in the process.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NoteId(Uuid);

impl NoteId {
    pub fn generate() -> Self {
        Self(Uuid::new_v4())
    }
}

impl From<Uuid> for NoteId {
    fn from(id: Uuid) -> Self {
        Self(id)
    }
}

impl From<NoteId> for Uuid {
    fn from(id: NoteId) -> Self {
        id.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Note {
    id: NoteId,
    text: NoteText,
    completed: bool,
}

impl Note {
    pub fn new(id: NoteId, text: NoteText, completed: bool) -> Self {
        Self {
            id,
            text,
            completed,
        }
    }

    pub fn id(&self) -> NoteId {
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
pub struct NoteNotFound;

pub trait NoteRepository: Clone + Send + Sync + 'static {
    fn create(&self, owner: &str, text: &NoteText) -> impl Future<Output = Note> + Send;

    fn list(&self, owner: &str) -> impl Future<Output = Vec<Note>> + Send;

    fn update(
        &self,
        owner: &str,
        id: NoteId,
        text: Option<&NoteText>,
        completed: Option<bool>,
    ) -> impl Future<Output = Result<Note, NoteNotFound>> + Send;

    fn delete(
        &self,
        owner: &str,
        id: NoteId,
    ) -> impl Future<Output = Result<(), NoteNotFound>> + Send;
}

pub trait NoteService: Clone + Send + Sync + 'static {
    fn create(&self, owner: &str, text: &NoteText) -> impl Future<Output = Note> + Send;

    fn list(&self, owner: &str) -> impl Future<Output = Vec<Note>> + Send;

    fn update(
        &self,
        owner: &str,
        id: NoteId,
        text: Option<&NoteText>,
        completed: Option<bool>,
    ) -> impl Future<Output = Result<Note, NoteNotFound>> + Send;

    fn delete(
        &self,
        owner: &str,
        id: NoteId,
    ) -> impl Future<Output = Result<(), NoteNotFound>> + Send;
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
    async fn create(&self, owner: &str, text: &NoteText) -> Note {
        self.repo.create(owner, text).await
    }

    async fn list(&self, owner: &str) -> Vec<Note> {
        self.repo.list(owner).await
    }

    async fn update(
        &self,
        owner: &str,
        id: NoteId,
        text: Option<&NoteText>,
        completed: Option<bool>,
    ) -> Result<Note, NoteNotFound> {
        self.repo.update(owner, id, text, completed).await
    }

    async fn delete(&self, owner: &str, id: NoteId) -> Result<(), NoteNotFound> {
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

    #[test]
    fn note_id_is_its_own_uuid() {
        let id = NoteId::generate();
        let raw = Uuid::from(id);
        assert_eq!(NoteId::from(raw), id);
    }
}
