use thiserror::Error;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct UserId(String);

#[derive(Clone, Debug, PartialEq, Eq, Error)]
#[error("user id cannot be empty")]
pub struct UserIdEmptyError;

impl UserId {
    pub fn new(raw: &str) -> Result<Self, UserIdEmptyError> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            Err(UserIdEmptyError)
        } else {
            Ok(Self(trimmed.to_string()))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct UserName(String);

#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum UserNameError {
    #[error("user name cannot be empty")]
    Empty,
    #[error("user name is longer than 32 characters")]
    TooLong,
    #[error("user name contains a character traQ does not allow")]
    Invalid,
}

impl UserName {
    pub fn new(raw: &str) -> Result<Self, UserNameError> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(UserNameError::Empty);
        }
        if trimmed.chars().count() > 32 {
            return Err(UserNameError::TooLong);
        }
        if !trimmed
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
        {
            return Err(UserNameError::Invalid);
        }
        Ok(Self(trimmed.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}
