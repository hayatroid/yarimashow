use crate::domain::auth::{AuthError, Authorization, IdentityProvider, SignedInUser};
use crate::domain::user::{UserId, UserName};

/// Identity provider that never leaves the process.
/// HTTP tests use it in place of traQ.
#[derive(Clone, Debug)]
pub struct FakeIdentity {
    state: String,
    code: String,
    user: SignedInUser,
}

impl FakeIdentity {
    pub fn new(state: &str, code: &str, user_id: &str, user_name: &str) -> Self {
        Self {
            state: state.to_string(),
            code: code.to_string(),
            user: SignedInUser::new(
                UserId::new(user_id).expect("fake user id"),
                UserName::new(user_name).expect("fake user name"),
            ),
        }
    }
}

impl IdentityProvider for FakeIdentity {
    fn authorization(&self) -> Result<Authorization, AuthError> {
        Ok(Authorization::new(
            format!(
                "https://q.trap.jp/api/v3/oauth2/authorize?state={}",
                self.state
            ),
            self.state.clone(),
        ))
    }

    async fn authenticate(&self, code: &str) -> Result<SignedInUser, AuthError> {
        if code == self.code {
            Ok(self.user.clone())
        } else {
            Err(AuthError::Rejected)
        }
    }
}
