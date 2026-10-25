use std::future::Future;

use anyhow::Error as AnyhowError;
use thiserror::Error;

use super::user::{UserId, UserName};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SignedInUser {
    id: UserId,
    name: UserName,
}

impl SignedInUser {
    pub fn new(id: UserId, name: UserName) -> Self {
        Self { id, name }
    }

    pub fn id(&self) -> &UserId {
        &self.id
    }

    pub fn name(&self) -> &UserName {
        &self.name
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Authorization {
    url: String,
    state: String,
}

impl Authorization {
    pub fn new(url: impl Into<String>, state: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            state: state.into(),
        }
    }

    pub fn url(&self) -> &str {
        &self.url
    }

    pub fn state(&self) -> &str {
        &self.state
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionRecord {
    csrf: Option<String>,
    user: Option<SignedInUser>,
}

impl SessionRecord {
    pub fn csrf(state: impl Into<String>) -> Self {
        Self {
            csrf: Some(state.into()),
            user: None,
        }
    }

    pub fn user(user: SignedInUser) -> Self {
        Self {
            csrf: None,
            user: Some(user),
        }
    }

    pub fn csrf_token(&self) -> Option<&str> {
        self.csrf.as_deref()
    }

    pub fn signed_in_user(&self) -> Option<&SignedInUser> {
        self.user.as_ref()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LoginStart {
    url: String,
    session_id: String,
}

impl LoginStart {
    pub fn url(&self) -> &str {
        &self.url
    }

    pub fn session_id(&self) -> &str {
        &self.session_id
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompletedLogin {
    session_id: String,
    user: SignedInUser,
}

impl CompletedLogin {
    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    pub fn user(&self) -> &SignedInUser {
        &self.user
    }
}

#[derive(Debug, Error)]
pub enum AuthError {
    #[error("login was rejected")]
    Rejected,
    #[error("login failed")]
    Unknown(#[source] AnyhowError),
}

pub trait IdentityProvider: Clone + Send + Sync + 'static {
    fn authorization(&self) -> Result<Authorization, AuthError>;
    fn authenticate(
        &self,
        code: &str,
    ) -> impl Future<Output = Result<SignedInUser, AuthError>> + Send;
}

pub trait Sessions: Clone + Send + Sync + 'static {
    fn create(
        &self,
        record: SessionRecord,
    ) -> impl Future<Output = Result<String, AuthError>> + Send;
    fn load(
        &self,
        session_id: &str,
    ) -> impl Future<Output = Result<Option<SessionRecord>, AuthError>> + Send;
    fn delete(&self, session_id: &str) -> impl Future<Output = Result<(), AuthError>> + Send;
}

pub trait AuthService: Clone + Send + Sync + 'static {
    fn begin(&self) -> impl Future<Output = Result<LoginStart, AuthError>> + Send;
    fn complete(
        &self,
        session_id: &str,
        code: &str,
        state: &str,
    ) -> impl Future<Output = Result<CompletedLogin, AuthError>> + Send;
    fn current(
        &self,
        session_id: &str,
    ) -> impl Future<Output = Result<Option<SignedInUser>, AuthError>> + Send;
    fn logout(&self, session_id: &str) -> impl Future<Output = Result<(), AuthError>> + Send;
}

#[derive(Debug, Clone)]
pub struct Service<I, S> {
    identity: I,
    sessions: S,
}

impl<I, S> Service<I, S> {
    pub fn new(identity: I, sessions: S) -> Self {
        Self { identity, sessions }
    }
}

impl<I, S> AuthService for Service<I, S>
where
    I: IdentityProvider,
    S: Sessions,
{
    async fn begin(&self) -> Result<LoginStart, AuthError> {
        let authorization = self.identity.authorization()?;
        let session_id = self
            .sessions
            .create(SessionRecord::csrf(authorization.state().to_string()))
            .await?;
        Ok(LoginStart {
            url: authorization.url().to_string(),
            session_id,
        })
    }

    async fn complete(
        &self,
        session_id: &str,
        code: &str,
        state: &str,
    ) -> Result<CompletedLogin, AuthError> {
        let record = self
            .sessions
            .load(session_id)
            .await?
            .ok_or(AuthError::Rejected)?;
        if record.csrf_token() != Some(state) {
            return Err(AuthError::Rejected);
        }
        let user = self.identity.authenticate(code).await?;
        self.sessions.delete(session_id).await?;
        let session_id = self
            .sessions
            .create(SessionRecord::user(user.clone()))
            .await?;
        Ok(CompletedLogin { session_id, user })
    }

    async fn current(&self, session_id: &str) -> Result<Option<SignedInUser>, AuthError> {
        let Some(record) = self.sessions.load(session_id).await? else {
            return Ok(None);
        };
        Ok(record.signed_in_user().cloned())
    }

    async fn logout(&self, session_id: &str) -> Result<(), AuthError> {
        self.sessions.delete(session_id).await
    }
}
