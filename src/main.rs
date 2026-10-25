use anyhow::Context;

use yarimashow::config::{Config, NotesBackend};
use yarimashow::domain::auth::Service as AuthServiceImpl;
use yarimashow::domain::note::Service as NoteServiceImpl;
use yarimashow::inbound::http::{HttpServer, HttpServerConfig};
use yarimashow::outbound::dynamodb::{DynamoNotes, DynamoSettings};
use yarimashow::outbound::memory::{MemoryNotes, MemorySessions};
use yarimashow::outbound::traq::TraqIdentity;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "yarimashow=info".parse().unwrap()),
        )
        .init();

    let config = Config::from_env()?;
    let identity = TraqIdentity::new(&config.traq)?;
    let auth = AuthServiceImpl::new(identity, MemorySessions::new());
    let http = HttpServerConfig {
        port: config.port,
        cookie_secure: config.cookie_secure,
    };

    match config.notes {
        NotesBackend::Memory => {
            let notes = NoteServiceImpl::new(MemoryNotes::new());
            HttpServer::new(notes, auth, http).run().await
        }
        NotesBackend::Dynamo {
            region,
            table,
            endpoint,
        } => {
            let notes = NoteServiceImpl::new(
                DynamoNotes::connect(&DynamoSettings {
                    region,
                    table,
                    endpoint,
                })
                .await
                .context("failed to configure DynamoDB")?,
            );
            HttpServer::new(notes, auth, http).run().await
        }
    }
}
