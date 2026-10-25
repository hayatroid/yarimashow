//! OAuth routes follow `axum/examples/oauth`. Note routes follow `axum/examples/todos`.
//! `/auth/discord` is `/auth/traq`, and the authorize, token, and user URLs are traQ's.

use std::convert::Infallible;
use std::env;
use std::time::Duration;

use anyhow::{anyhow, Context};
use async_session::{MemoryStore, Session, SessionStore};
use axum::error_handling::HandleErrorLayer;
use axum::extract::{FromRef, FromRequestParts, OptionalFromRequestParts, Path, Query, State};
use axum::http::header::SET_COOKIE;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{get, patch};
use axum::{Json, RequestPartsExt, Router};
use axum_extra::headers::Cookie;
use axum_extra::typed_header::TypedHeaderRejectionReason;
use axum_extra::TypedHeader;
use http::header;
use http::request::Parts;
use oauth2::{
    AuthUrl, AuthorizationCode, ClientId, ClientSecret, CsrfToken, EndpointNotSet, EndpointSet,
    RedirectUrl, Scope, TokenResponse, TokenUrl,
};
use serde::{Deserialize, Serialize};
use tower::BoxError;
use tower::ServiceBuilder;
use tower_http::trace::TraceLayer;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use uuid::Uuid;

use crate::domain::note::{Note, NoteId, NoteNotFound, NoteService, NoteText, Service};
use crate::outbound::memory::MemoryNotes;

const COOKIE_NAME: &str = "SESSION";
const CSRF_TOKEN: &str = "csrf_token";
const USERINFO_URL: &str = "https://q.trap.jp/api/v3/users/me";

type BasicClient = oauth2::basic::BasicClient<
    EndpointSet,
    EndpointNotSet,
    EndpointNotSet,
    EndpointNotSet,
    EndpointSet,
>;

type Notes = Service<MemoryNotes>;

#[derive(Clone)]
struct AppState {
    store: MemoryStore,
    oauth_client: BasicClient,
    notes: Notes,
}

impl FromRef<AppState> for MemoryStore {
    fn from_ref(state: &AppState) -> Self {
        state.store.clone()
    }
}

impl FromRef<AppState> for BasicClient {
    fn from_ref(state: &AppState) -> Self {
        state.oauth_client.clone()
    }
}

pub async fn serve() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| format!("{}=debug", env!("CARGO_CRATE_NAME")).into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .map_err(|err| anyhow::anyhow!("failed to bind TcpListener: {err}"))?;
    tracing::debug!("listening on {}", listener.local_addr()?);
    axum::serve(listener, app(AppState::from_env()?)).await?;
    Ok(())
}

impl AppState {
    fn from_env() -> anyhow::Result<Self> {
        Ok(Self {
            store: MemoryStore::new(),
            oauth_client: oauth_client().map_err(|err| err.0)?,
            notes: Service::new(MemoryNotes::new()),
        })
    }
}

fn oauth_client() -> Result<BasicClient, AppError> {
    let client_id = env::var("CLIENT_ID").context("Missing CLIENT_ID!")?;
    let client_secret = env::var("CLIENT_SECRET").context("Missing CLIENT_SECRET!")?;
    let redirect_url = env::var("REDIRECT_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:3000/auth/authorized".to_string());
    let auth_url = env::var("AUTH_URL").unwrap_or_else(|_| {
        "https://q.trap.jp/api/v3/oauth2/authorize?response_type=code".to_string()
    });
    let token_url = env::var("TOKEN_URL")
        .unwrap_or_else(|_| "https://q.trap.jp/api/v3/oauth2/token".to_string());

    Ok(oauth2::basic::BasicClient::new(ClientId::new(client_id))
        .set_client_secret(ClientSecret::new(client_secret))
        .set_auth_uri(
            AuthUrl::new(auth_url).context("failed to create new authorization server URL")?,
        )
        .set_token_uri(TokenUrl::new(token_url).context("failed to create new token endpoint URL")?)
        .set_redirect_uri(
            RedirectUrl::new(redirect_url).context("failed to create new redirection URL")?,
        ))
}

fn app(state: AppState) -> Router {
    Router::new()
        .route("/", get(index))
        .route("/auth/traq", get(traq_auth))
        .route("/auth/authorized", get(login_authorized))
        .route("/protected", get(protected))
        .route("/logout", get(logout))
        .route("/notes", get(notes_index).post(notes_create))
        .route("/notes/{id}", patch(notes_update).delete(notes_delete))
        .layer(
            ServiceBuilder::new()
                .layer(HandleErrorLayer::new(|error: BoxError| async move {
                    if error.is::<tower::timeout::error::Elapsed>() {
                        Ok(StatusCode::REQUEST_TIMEOUT)
                    } else {
                        Err((
                            StatusCode::INTERNAL_SERVER_ERROR,
                            format!("Unhandled internal error: {error}"),
                        ))
                    }
                }))
                .timeout(Duration::from_secs(10))
                .layer(TraceLayer::new_for_http())
                .into_inner(),
        )
        .with_state(state)
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct User {
    id: String,
    name: String,
}

async fn index(user: Option<User>) -> impl IntoResponse {
    match user {
        Some(user) => format!(
            "Hey {}! You're logged in!\nYou may now access `/notes`.\nLog out with `/logout`.",
            user.name
        ),
        None => "You're not logged in.\nVisit `/auth/traq` to do so.".to_string(),
    }
}

async fn traq_auth(
    State(client): State<BasicClient>,
    State(store): State<MemoryStore>,
) -> Result<(HeaderMap, Redirect), AppError> {
    let (auth_url, csrf_token) = client
        .authorize_url(CsrfToken::new_random)
        .add_scope(Scope::new("read".to_string()))
        .url();

    let mut session = Session::new();
    session
        .insert(CSRF_TOKEN, &csrf_token)
        .context("failed in inserting CSRF token into session")?;
    let cookie = store
        .store_session(session)
        .await
        .context("failed to store CSRF token session")?
        .context("unexpected error retrieving CSRF cookie value")?;
    let mut headers = HeaderMap::new();
    let cookie = format!("{COOKIE_NAME}={cookie}; SameSite=Lax; HttpOnly; Secure; Path=/");
    headers.insert(
        SET_COOKIE,
        cookie.parse().context("failed to parse cookie")?,
    );
    Ok((headers, Redirect::to(auth_url.as_str())))
}

async fn protected(user: User) -> impl IntoResponse {
    format!("Welcome to the protected area :)\nHere's your info:\n{user:?}")
}

async fn logout(
    State(store): State<MemoryStore>,
    TypedHeader(cookies): TypedHeader<Cookie>,
) -> Result<Redirect, AppError> {
    let cookie = cookies
        .get(COOKIE_NAME)
        .context("unexpected error getting cookie name")?;
    let session = match store
        .load_session(cookie.to_string())
        .await
        .context("failed to load session")?
    {
        Some(session) => session,
        None => return Ok(Redirect::to("/")),
    };
    store
        .destroy_session(session)
        .await
        .context("failed to destroy session")?;
    Ok(Redirect::to("/"))
}

#[derive(Debug, Deserialize)]
struct AuthRequest {
    code: String,
    state: String,
}

async fn csrf_token_validation_workflow(
    auth_request: &AuthRequest,
    cookies: &Cookie,
    store: &MemoryStore,
) -> Result<(), AppError> {
    let cookie = cookies
        .get(COOKIE_NAME)
        .context("unexpected error getting cookie name")?
        .to_string();
    let session = match store
        .load_session(cookie)
        .await
        .context("failed to load session")?
    {
        Some(session) => session,
        None => return Err(anyhow!("Session not found").into()),
    };
    let stored_csrf_token = session
        .get::<CsrfToken>(CSRF_TOKEN)
        .context("CSRF token not found in session")?
        .to_owned();
    store
        .destroy_session(session)
        .await
        .context("Failed to destroy old session")?;
    if *stored_csrf_token.secret() != auth_request.state {
        return Err(anyhow!("CSRF token mismatch").into());
    }
    Ok(())
}

async fn login_authorized(
    Query(query): Query<AuthRequest>,
    State(store): State<MemoryStore>,
    State(oauth_client): State<BasicClient>,
    TypedHeader(cookies): TypedHeader<Cookie>,
) -> Result<(HeaderMap, Redirect), AppError> {
    csrf_token_validation_workflow(&query, &cookies, &store).await?;

    let client = reqwest::Client::new();
    let token = oauth_client
        .exchange_code(AuthorizationCode::new(query.code))
        .request_async(&client)
        .await
        .context("failed in sending request to authorization server")?;
    let user = client
        .get(USERINFO_URL)
        .bearer_auth(token.access_token().secret())
        .send()
        .await
        .context("failed in sending request to target Url")?
        .json::<User>()
        .await
        .context("failed to deserialize response as JSON")?;

    let mut session = Session::new();
    session
        .insert("user", &user)
        .context("failed in inserting serialized value into session")?;
    let cookie = store
        .store_session(session)
        .await
        .context("failed to store session")?
        .context("unexpected error retrieving cookie value")?;
    let mut headers = HeaderMap::new();
    let cookie = format!("{COOKIE_NAME}={cookie}; SameSite=Lax; HttpOnly; Secure; Path=/");
    headers.insert(
        SET_COOKIE,
        cookie.parse().context("failed to parse cookie")?,
    );
    Ok((headers, Redirect::to("/")))
}

struct AuthRedirect;

impl IntoResponse for AuthRedirect {
    fn into_response(self) -> Response {
        Redirect::temporary("/auth/traq").into_response()
    }
}

impl<S> FromRequestParts<S> for User
where
    MemoryStore: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = AuthRedirect;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let store = MemoryStore::from_ref(state);
        let cookies = parts
            .extract::<TypedHeader<Cookie>>()
            .await
            .map_err(|err| match *err.name() {
                header::COOKIE => match err.reason() {
                    TypedHeaderRejectionReason::Missing => AuthRedirect,
                    _ => panic!("unexpected error getting Cookie header(s): {err}"),
                },
                _ => panic!("unexpected error getting cookies: {err}"),
            })?;
        let session_cookie = cookies.get(COOKIE_NAME).ok_or(AuthRedirect)?;
        let session = store
            .load_session(session_cookie.to_string())
            .await
            .unwrap()
            .ok_or(AuthRedirect)?;
        session.get::<User>("user").ok_or(AuthRedirect)
    }
}

impl<S> OptionalFromRequestParts<S> for User
where
    MemoryStore: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = Infallible;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &S,
    ) -> Result<Option<Self>, Self::Rejection> {
        match <User as FromRequestParts<S>>::from_request_parts(parts, state).await {
            Ok(user) => Ok(Some(user)),
            Err(AuthRedirect) => Ok(None),
        }
    }
}

#[derive(Debug)]
struct AppError(anyhow::Error);

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        tracing::error!("Application error: {:#}", self.0);
        (StatusCode::INTERNAL_SERVER_ERROR, "Something went wrong").into_response()
    }
}

impl<E> From<E> for AppError
where
    E: Into<anyhow::Error>,
{
    fn from(err: E) -> Self {
        Self(err.into())
    }
}

#[derive(Debug, Deserialize, Default)]
struct Pagination {
    offset: Option<usize>,
    limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct CreateNote {
    text: String,
}

#[derive(Debug, Deserialize)]
struct UpdateNote {
    text: Option<String>,
    completed: Option<bool>,
}

#[derive(Serialize)]
struct NoteResponse {
    id: Uuid,
    text: String,
    completed: bool,
}

impl From<Note> for NoteResponse {
    fn from(note: Note) -> Self {
        Self {
            id: Uuid::from(note.id()),
            text: note.text().as_str().to_string(),
            completed: note.completed(),
        }
    }
}

async fn notes_index(
    user: User,
    Query(pagination): Query<Pagination>,
    State(state): State<AppState>,
) -> Result<Json<Vec<NoteResponse>>, StatusCode> {
    let notes = state.notes.list(&user.id).await;
    let notes = notes
        .into_iter()
        .skip(pagination.offset.unwrap_or(0))
        .take(pagination.limit.unwrap_or(usize::MAX))
        .map(NoteResponse::from)
        .collect();
    Ok(Json(notes))
}

async fn notes_create(
    user: User,
    State(state): State<AppState>,
    Json(input): Json<CreateNote>,
) -> Result<(StatusCode, Json<NoteResponse>), StatusCode> {
    let text = NoteText::new(&input.text).map_err(|_| StatusCode::UNPROCESSABLE_ENTITY)?;
    let note = state.notes.create(&user.id, &text).await;
    Ok((StatusCode::CREATED, Json(note.into())))
}

async fn notes_update(
    user: User,
    Path(id): Path<Uuid>,
    State(state): State<AppState>,
    Json(input): Json<UpdateNote>,
) -> Result<Json<NoteResponse>, StatusCode> {
    let text = input
        .text
        .as_deref()
        .map(NoteText::new)
        .transpose()
        .map_err(|_| StatusCode::UNPROCESSABLE_ENTITY)?;
    let note = state
        .notes
        .update(&user.id, NoteId::from(id), text.as_ref(), input.completed)
        .await
        .map_err(|NoteNotFound| StatusCode::NOT_FOUND)?;
    Ok(Json(note.into()))
}

async fn notes_delete(
    user: User,
    Path(id): Path<Uuid>,
    State(state): State<AppState>,
) -> Result<StatusCode, StatusCode> {
    state
        .notes
        .delete(&user.id, NoteId::from(id))
        .await
        .map_err(|NoteNotFound| StatusCode::NOT_FOUND)?;
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use axum::body::Body;
    use http::{Request, StatusCode};
    use http_body_util::BodyExt;
    use oauth2::{AuthUrl, ClientId, ClientSecret, RedirectUrl, TokenUrl};
    use serde_json::{json, Value};
    use tower::ServiceExt;

    use super::*;

    fn test_state() -> AppState {
        let oauth_client = oauth2::basic::BasicClient::new(ClientId::new("cid".into()))
            .set_client_secret(ClientSecret::new("secret".into()))
            .set_auth_uri(
                AuthUrl::new("https://q.trap.jp/api/v3/oauth2/authorize?response_type=code".into())
                    .unwrap(),
            )
            .set_token_uri(TokenUrl::new("https://q.trap.jp/api/v3/oauth2/token".into()).unwrap())
            .set_redirect_uri(
                RedirectUrl::new("http://127.0.0.1:3000/auth/authorized".into()).unwrap(),
            );
        AppState {
            store: MemoryStore::new(),
            oauth_client,
            notes: Service::new(MemoryNotes::new()),
        }
    }

    async fn login_cookie(state: &AppState, id: &str, name: &str) -> String {
        let mut session = Session::new();
        session
            .insert(
                "user",
                &User {
                    id: id.to_string(),
                    name: name.to_string(),
                },
            )
            .unwrap();
        let value = state.store.store_session(session).await.unwrap().unwrap();
        format!("{COOKIE_NAME}={value}")
    }

    async fn json_body(response: Response) -> Value {
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        serde_json::from_slice(&bytes).unwrap()
    }

    #[test]
    fn authorize_url_points_at_traq() {
        let (url, _) = test_state()
            .oauth_client
            .authorize_url(CsrfToken::new_random)
            .add_scope(oauth2::Scope::new("read".to_string()))
            .url();
        let url = url.as_str();
        assert!(url.starts_with("https://q.trap.jp/api/v3/oauth2/authorize?"));
        assert!(url.contains("client_id=cid"));
        assert!(url.contains("scope=read"));
    }

    #[tokio::test]
    async fn notes_redirect_until_login_and_stay_per_user() {
        let state = test_state();
        let app = app(state.clone());

        let response = app
            .clone()
            .oneshot(Request::get("/notes").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::TEMPORARY_REDIRECT);
        assert_eq!(
            response.headers().get(header::LOCATION).unwrap(),
            "/auth/traq"
        );

        let alice = login_cookie(&state, "alice", "alice").await;
        let created = app
            .clone()
            .oneshot(
                Request::post("/notes")
                    .header(header::COOKIE, &alice)
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(r#"{"text":"  first memo  "}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(created.status(), StatusCode::CREATED);
        let created = json_body(created).await;
        assert_eq!(created["text"], "first memo");
        assert_eq!(created["completed"], false);

        let blank = app
            .clone()
            .oneshot(
                Request::post("/notes")
                    .header(header::COOKIE, &alice)
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(r#"{"text":"   "}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(blank.status(), StatusCode::UNPROCESSABLE_ENTITY);

        let bob = login_cookie(&state, "bob", "bob").await;
        let bobs = app
            .clone()
            .oneshot(
                Request::get("/notes")
                    .header(header::COOKIE, &bob)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(json_body(bobs).await, json!([]));

        let alices = app
            .oneshot(
                Request::get("/notes")
                    .header(header::COOKIE, &alice)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let alices = json_body(alices).await;
        assert_eq!(alices.as_array().unwrap().len(), 1);
        assert_eq!(alices[0]["text"], "first memo");
    }

    #[tokio::test]
    async fn csrf_mismatch_is_rejected() {
        let state = test_state();
        let mut session = Session::new();
        session
            .insert(CSRF_TOKEN, CsrfToken::new("expected".into()))
            .unwrap();
        let value = state.store.store_session(session).await.unwrap().unwrap();
        let response = app(state)
            .oneshot(
                Request::get("/auth/authorized?code=nope&state=other")
                    .header(header::COOKIE, format!("{COOKIE_NAME}={value}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }
}
