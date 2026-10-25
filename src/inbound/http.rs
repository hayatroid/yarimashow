use anyhow::Context;
use axum::extract::{Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use crate::domain::auth::{AuthError, AuthService, SignedInUser};
use crate::domain::note::{
    CreateNoteError, ListNotesError, Note, NoteBody, NoteBodyError, NoteService,
};

#[derive(Clone, Debug)]
pub struct HttpServerConfig {
    pub port: u16,
    pub cookie_secure: bool,
}

#[derive(Clone)]
struct AppState<N, A> {
    notes: N,
    auth: A,
    cookie_secure: bool,
}

pub struct HttpServer<N, A> {
    state: AppState<N, A>,
    port: u16,
}

impl<N, A> HttpServer<N, A>
where
    N: NoteService,
    A: AuthService,
{
    pub fn new(notes: N, auth: A, config: HttpServerConfig) -> Self {
        Self {
            state: AppState {
                notes,
                auth,
                cookie_secure: config.cookie_secure,
            },
            port: config.port,
        }
    }

    pub fn router(&self) -> Router {
        Router::new()
            .route("/", get(index::<N, A>))
            .route("/auth/traq", get(traq_auth::<N, A>))
            .route("/auth/authorized", get(authorized::<N, A>))
            .route("/logout", get(logout::<N, A>))
            .route("/notes", get(list_notes::<N, A>).post(create_note::<N, A>))
            .with_state(self.state.clone())
    }

    pub async fn run(self) -> anyhow::Result<()> {
        let port = self.port;
        let listener = tokio::net::TcpListener::bind(("0.0.0.0", port))
            .await
            .with_context(|| format!("failed to bind 0.0.0.0:{port}"))?;
        tracing::info!("listening on {}", listener.local_addr()?);
        axum::serve(listener, self.router())
            .await
            .context("http server stopped")?;
        Ok(())
    }
}

async fn index<N, A>(State(state): State<AppState<N, A>>, headers: HeaderMap) -> Response
where
    N: NoteService,
    A: AuthService,
{
    let user = match signed_in_user(&state, &headers).await {
        Ok(user) => user,
        Err(err) => return err.into_response(),
    };
    Json(IndexBody {
        user: user.as_ref().map(UserBody::from),
    })
    .into_response()
}

async fn traq_auth<N, A>(State(state): State<AppState<N, A>>) -> Result<Response, ApiError>
where
    N: NoteService,
    A: AuthService,
{
    let start = state.auth.begin().await?;
    let mut headers = session_cookie(start.session_id(), state.cookie_secure);
    headers.insert(
        header::LOCATION,
        start.url().parse().map_err(|_| {
            ApiError::Internal(anyhow::anyhow!("authorization url is not a valid header"))
        })?,
    );
    Ok((StatusCode::SEE_OTHER, headers).into_response())
}

#[derive(Debug, Deserialize)]
struct AuthQuery {
    code: String,
    state: String,
}

async fn authorized<N, A>(
    State(state): State<AppState<N, A>>,
    headers: HeaderMap,
    Query(query): Query<AuthQuery>,
) -> Result<Response, ApiError>
where
    N: NoteService,
    A: AuthService,
{
    let session_id = session_id(&headers).ok_or(ApiError::BadRequest("login was rejected"))?;
    let completed = state
        .auth
        .complete(session_id, &query.code, &query.state)
        .await?;
    let mut headers = session_cookie(completed.session_id(), state.cookie_secure);
    headers.insert(
        header::LOCATION,
        "/".parse().expect("root is a valid location"),
    );
    Ok((StatusCode::SEE_OTHER, headers).into_response())
}

async fn logout<N, A>(
    State(state): State<AppState<N, A>>,
    headers: HeaderMap,
) -> Result<Response, ApiError>
where
    N: NoteService,
    A: AuthService,
{
    if let Some(session_id) = session_id(&headers) {
        state.auth.logout(session_id).await?;
    }
    let mut headers = HeaderMap::new();
    headers.insert(
        header::SET_COOKIE,
        "SESSION=; Max-Age=0; HttpOnly; Path=/; SameSite=Lax"
            .parse()
            .expect("clear cookie is valid"),
    );
    headers.insert(
        header::LOCATION,
        "/".parse().expect("root is a valid location"),
    );
    Ok((StatusCode::SEE_OTHER, headers).into_response())
}

#[derive(Debug, Deserialize)]
struct CreateNoteRequest {
    body: String,
}

async fn create_note<N, A>(
    State(state): State<AppState<N, A>>,
    headers: HeaderMap,
    Json(request): Json<CreateNoteRequest>,
) -> Result<(StatusCode, Json<NoteBodyResponse>), ApiError>
where
    N: NoteService,
    A: AuthService,
{
    let user = require_user(&state, &headers).await?;
    let body = NoteBody::new(&request.body).map_err(ApiError::from)?;
    let note = state.notes.create(user.id(), &body).await?;
    Ok((StatusCode::CREATED, Json(NoteBodyResponse::from(&note))))
}

async fn list_notes<N, A>(
    State(state): State<AppState<N, A>>,
    headers: HeaderMap,
) -> Result<Json<NotesBody>, ApiError>
where
    N: NoteService,
    A: AuthService,
{
    let user = require_user(&state, &headers).await?;
    let notes = state.notes.list(user.id()).await?;
    Ok(Json(NotesBody {
        notes: notes.iter().map(NoteBodyResponse::from).collect(),
    }))
}

async fn require_user<N, A>(
    state: &AppState<N, A>,
    headers: &HeaderMap,
) -> Result<SignedInUser, ApiError>
where
    N: NoteService,
    A: AuthService,
{
    signed_in_user(state, headers)
        .await?
        .ok_or(ApiError::Unauthorized)
}

async fn signed_in_user<N, A>(
    state: &AppState<N, A>,
    headers: &HeaderMap,
) -> Result<Option<SignedInUser>, ApiError>
where
    N: NoteService,
    A: AuthService,
{
    let Some(session_id) = session_id(headers) else {
        return Ok(None);
    };
    state.auth.current(session_id).await.map_err(ApiError::from)
}

fn session_id(headers: &HeaderMap) -> Option<&str> {
    let value = headers.get(header::COOKIE)?.to_str().ok()?;
    value.split(';').find_map(|part| {
        let (name, raw) = part.trim().split_once('=')?;
        (name == "SESSION" && !raw.is_empty()).then_some(raw)
    })
}

fn session_cookie(value: &str, secure: bool) -> HeaderMap {
    let mut cookie = format!("SESSION={value}; SameSite=Lax; HttpOnly; Path=/");
    if secure {
        cookie.push_str("; Secure");
    }
    let mut headers = HeaderMap::new();
    headers.insert(
        header::SET_COOKIE,
        cookie.parse().expect("session cookie is valid"),
    );
    headers
}

#[derive(Serialize)]
struct IndexBody {
    user: Option<UserBody>,
}

#[derive(Serialize)]
struct UserBody {
    id: String,
    name: String,
}

impl From<&SignedInUser> for UserBody {
    fn from(user: &SignedInUser) -> Self {
        Self {
            id: user.id().as_str().to_string(),
            name: user.name().as_str().to_string(),
        }
    }
}

#[derive(Serialize)]
struct NotesBody {
    notes: Vec<NoteBodyResponse>,
}

#[derive(Serialize)]
struct NoteBodyResponse {
    id: String,
    body: String,
    created_at_ms: i64,
}

impl From<&Note> for NoteBodyResponse {
    fn from(note: &Note) -> Self {
        Self {
            id: note.id().to_string(),
            body: note.body().as_str().to_string(),
            created_at_ms: note.created_at_ms(),
        }
    }
}

enum ApiError {
    BadRequest(&'static str),
    Unauthorized,
    Unprocessable(String),
    Internal(anyhow::Error),
}

impl From<AuthError> for ApiError {
    fn from(err: AuthError) -> Self {
        match err {
            AuthError::Rejected => Self::BadRequest("login was rejected"),
            AuthError::Unknown(err) => Self::Internal(err),
        }
    }
}

impl From<NoteBodyError> for ApiError {
    fn from(err: NoteBodyError) -> Self {
        Self::Unprocessable(err.to_string())
    }
}

impl From<CreateNoteError> for ApiError {
    fn from(err: CreateNoteError) -> Self {
        match err {
            CreateNoteError::Unknown(err) => Self::Internal(err),
        }
    }
}

impl From<ListNotesError> for ApiError {
    fn from(err: ListNotesError) -> Self {
        match err {
            ListNotesError::Unknown(err) => Self::Internal(err),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            Self::BadRequest(message) => (StatusCode::BAD_REQUEST, message.to_string()),
            Self::Unauthorized => (StatusCode::UNAUTHORIZED, "login required".to_string()),
            Self::Unprocessable(message) => (StatusCode::UNPROCESSABLE_ENTITY, message),
            Self::Internal(err) => {
                tracing::error!("{err:#}");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "something went wrong".to_string(),
                )
            }
        };
        (status, Json(serde_json::json!({ "error": message }))).into_response()
    }
}

#[cfg(test)]
mod tests {
    use std::env;

    use axum::body::Body;
    use http::Request;
    use http_body_util::BodyExt;
    use serde_json::{json, Value};
    use tower::ServiceExt;

    use super::*;
    use crate::domain::auth::Service as AuthServiceImpl;
    use crate::domain::note::Service as NoteServiceImpl;
    use crate::outbound::dynamodb::{DynamoNotes, DynamoSettings};
    use crate::outbound::fake::FakeIdentity;
    use crate::outbound::memory::{MemoryNotes, MemorySessions};

    fn app<N>(notes: N) -> Router
    where
        N: NoteService,
    {
        HttpServer::new(
            notes,
            AuthServiceImpl::new(
                FakeIdentity::new("csrf-1", "good-code", "user-1", "hayat"),
                MemorySessions::new(),
            ),
            HttpServerConfig {
                port: 0,
                cookie_secure: false,
            },
        )
        .router()
    }

    async fn response_json(response: Response) -> (StatusCode, HeaderMap, Value) {
        let status = response.status();
        let headers = response.headers().clone();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let value = if bytes.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(&bytes).unwrap()
        };
        (status, headers, value)
    }

    fn cookie_from(headers: &HeaderMap) -> String {
        headers
            .get(header::SET_COOKIE)
            .unwrap()
            .to_str()
            .unwrap()
            .split(';')
            .next()
            .unwrap()
            .to_string()
    }

    async fn login(app: &Router) -> String {
        let response = app
            .clone()
            .oneshot(Request::get("/auth/traq").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::SEE_OTHER);
        let location = response
            .headers()
            .get(header::LOCATION)
            .unwrap()
            .to_str()
            .unwrap();
        assert!(location.contains("state=csrf-1"));
        let csrf_cookie = cookie_from(response.headers());

        let response = app
            .clone()
            .oneshot(
                Request::get("/auth/authorized?code=good-code&state=csrf-1")
                    .header(header::COOKIE, &csrf_cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::SEE_OTHER);
        cookie_from(response.headers())
    }

    async fn write_and_read(app: Router) {
        let (status, _, body) = response_json(
            app.clone()
                .oneshot(Request::get("/").body(Body::empty()).unwrap())
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, json!({ "user": null }));

        let (status, _, body) = response_json(
            app.clone()
                .oneshot(Request::get("/notes").body(Body::empty()).unwrap())
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["error"], "login required");

        let session = login(&app).await;
        let (status, _, body) = response_json(
            app.clone()
                .oneshot(
                    Request::get("/")
                        .header(header::COOKIE, &session)
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["user"]["id"], "user-1");
        assert_eq!(body["user"]["name"], "hayat");

        let (status, _, existing) = response_json(
            app.clone()
                .oneshot(
                    Request::get("/notes")
                        .header(header::COOKIE, &session)
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let before = existing["notes"].as_array().unwrap().len();

        let (status, _, body) = response_json(
            app.clone()
                .oneshot(
                    Request::post("/notes")
                        .header(header::COOKIE, &session)
                        .header(header::CONTENT_TYPE, "application/json")
                        .body(Body::from(r#"{"body":"  first memo  "}"#))
                        .unwrap(),
                )
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(body["body"], "first memo");

        let (status, _, _) = response_json(
            app.clone()
                .oneshot(
                    Request::post("/notes")
                        .header(header::COOKIE, &session)
                        .header(header::CONTENT_TYPE, "application/json")
                        .body(Body::from(r#"{"body":"   "}"#))
                        .unwrap(),
                )
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);

        let (status, _, body) = response_json(
            app.clone()
                .oneshot(
                    Request::get("/notes")
                        .header(header::COOKIE, &session)
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let notes = body["notes"].as_array().unwrap();
        assert_eq!(notes.len(), before + 1);
        assert_eq!(notes.last().unwrap()["body"], "first memo");

        let response = app
            .clone()
            .oneshot(
                Request::get("/logout")
                    .header(header::COOKIE, &session)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::SEE_OTHER);

        let (status, _, _) = response_json(
            app.oneshot(
                Request::get("/notes")
                    .header(header::COOKIE, &session)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn memory_notes_follow_the_login_flow() {
        write_and_read(app(NoteServiceImpl::new(MemoryNotes::new()))).await;
    }

    #[tokio::test]
    async fn mismatched_state_is_rejected() {
        let app = app(NoteServiceImpl::new(MemoryNotes::new()));
        let started = app
            .clone()
            .oneshot(Request::get("/auth/traq").body(Body::empty()).unwrap())
            .await
            .unwrap();
        let csrf_cookie = cookie_from(started.headers());
        let (status, _, body) = response_json(
            app.oneshot(
                Request::get("/auth/authorized?code=good-code&state=nope")
                    .header(header::COOKIE, csrf_cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["error"], "login was rejected");
    }

    #[tokio::test]
    #[ignore = "needs MiniStack and the yarimashow-notes table"]
    async fn ministack_notes_follow_the_login_flow() {
        let endpoint = env::var("MINISTACK_ENDPOINT").expect("MINISTACK_ENDPOINT");
        let repo = DynamoNotes::connect(&DynamoSettings {
            region: "ap-northeast-1".to_string(),
            table: "yarimashow-notes".to_string(),
            endpoint: Some(endpoint),
        })
        .await
        .unwrap();
        write_and_read(app(NoteServiceImpl::new(repo))).await;
    }
}
