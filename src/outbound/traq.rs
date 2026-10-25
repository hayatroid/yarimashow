use anyhow::Context;
use oauth2::{
    AuthUrl, AuthorizationCode, ClientId, ClientSecret, CsrfToken, EndpointNotSet, EndpointSet,
    RedirectUrl, Scope, TokenResponse, TokenUrl,
};
use serde::Deserialize;

use crate::config::TraqSettings;
use crate::domain::auth::{AuthError, Authorization, IdentityProvider, SignedInUser};
use crate::domain::user::{UserId, UserName};

type BasicClient = oauth2::basic::BasicClient<
    EndpointSet,
    EndpointNotSet,
    EndpointNotSet,
    EndpointNotSet,
    EndpointSet,
>;

#[derive(Clone)]
pub struct TraqIdentity {
    client: BasicClient,
    http: reqwest::Client,
    userinfo_url: String,
    scope: String,
}

impl TraqIdentity {
    pub fn new(settings: &TraqSettings) -> anyhow::Result<Self> {
        let client = oauth2::basic::BasicClient::new(ClientId::new(settings.client_id.clone()))
            .set_client_secret(ClientSecret::new(settings.client_secret.clone()))
            .set_auth_uri(
                AuthUrl::new(settings.auth_url.clone()).context("AUTH_URL is not a valid URL")?,
            )
            .set_token_uri(
                TokenUrl::new(settings.token_url.clone())
                    .context("TOKEN_URL is not a valid URL")?,
            )
            .set_redirect_uri(
                RedirectUrl::new(settings.redirect_url.clone())
                    .context("REDIRECT_URL is not a valid URL")?,
            );
        Ok(Self {
            client,
            http: reqwest::Client::new(),
            userinfo_url: settings.userinfo_url.clone(),
            scope: settings.scope.clone(),
        })
    }
}

impl IdentityProvider for TraqIdentity {
    fn authorization(&self) -> Result<Authorization, AuthError> {
        let (url, csrf) = self
            .client
            .authorize_url(CsrfToken::new_random)
            .add_scope(Scope::new(self.scope.clone()))
            .url();
        Ok(Authorization::new(url.as_str(), csrf.secret()))
    }

    async fn authenticate(&self, code: &str) -> Result<SignedInUser, AuthError> {
        let token = self
            .client
            .exchange_code(AuthorizationCode::new(code.to_string()))
            .request_async(&self.http)
            .await
            .context("traQ token request failed")
            .map_err(AuthError::Unknown)?;

        let profile: TraqMe = self
            .http
            .get(&self.userinfo_url)
            .bearer_auth(token.access_token().secret())
            .send()
            .await
            .context("traQ user request failed")
            .map_err(AuthError::Unknown)?
            .error_for_status()
            .context("traQ user request was rejected")
            .map_err(AuthError::Unknown)?
            .json()
            .await
            .context("traQ user response was not JSON")
            .map_err(AuthError::Unknown)?;

        let id = UserId::new(&profile.id).map_err(|err| AuthError::Unknown(err.into()))?;
        let name = UserName::new(&profile.name).map_err(|err| AuthError::Unknown(err.into()))?;
        Ok(SignedInUser::new(id, name))
    }
}

#[derive(Debug, Deserialize)]
struct TraqMe {
    id: String,
    name: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authorization_url_points_at_traq() {
        let identity = TraqIdentity::new(&TraqSettings {
            client_id: "cid".to_string(),
            client_secret: "secret".to_string(),
            redirect_url: "http://127.0.0.1:3000/auth/authorized".to_string(),
            auth_url: "https://q.trap.jp/api/v3/oauth2/authorize".to_string(),
            token_url: "https://q.trap.jp/api/v3/oauth2/token".to_string(),
            userinfo_url: "https://q.trap.jp/api/v3/users/me".to_string(),
            scope: "read".to_string(),
        })
        .unwrap();

        let authorization = identity.authorization().unwrap();
        assert!(authorization
            .url()
            .starts_with("https://q.trap.jp/api/v3/oauth2/authorize?"));
        assert!(authorization.url().contains("client_id=cid"));
        assert!(authorization.url().contains("redirect_uri="));
        assert!(!authorization.state().is_empty());
    }
}
