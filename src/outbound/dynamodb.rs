use std::collections::HashMap;

use aws_sdk_dynamodb::types::AttributeValue;
use aws_sdk_dynamodb::Client;

use crate::domain::note::{
    CreateNoteError, ListNotesError, Note, NoteBody, NoteId, NoteRepository,
};
use crate::domain::user::UserId;

#[derive(Clone, Debug)]
pub struct DynamoSettings {
    pub region: String,
    pub table: String,
    pub endpoint: Option<String>,
}

#[derive(Clone)]
pub struct DynamoNotes {
    client: Client,
    table: String,
}

impl DynamoNotes {
    pub async fn connect(settings: &DynamoSettings) -> anyhow::Result<Self> {
        let mut loader = aws_config::defaults(aws_config::BehaviorVersion::latest())
            .region(aws_config::Region::new(settings.region.clone()));
        if let Some(endpoint) = &settings.endpoint {
            loader = loader.endpoint_url(endpoint).credentials_provider(
                aws_credential_types::Credentials::new(
                    "test",
                    "test",
                    None::<String>,
                    None,
                    "ministack",
                ),
            );
        }
        let config = loader.load().await;
        Ok(Self {
            client: Client::new(&config),
            table: settings.table.clone(),
        })
    }
}

impl NoteRepository for DynamoNotes {
    async fn insert(&self, note: &Note) -> Result<(), CreateNoteError> {
        self.client
            .put_item()
            .table_name(&self.table)
            .item(
                "user_id",
                AttributeValue::S(note.owner().as_str().to_string()),
            )
            .item("note_id", AttributeValue::S(note.id().as_str()))
            .item("body", AttributeValue::S(note.body().as_str().to_string()))
            .item(
                "created_at_ms",
                AttributeValue::N(note.created_at_ms().to_string()),
            )
            .send()
            .await
            .map_err(|err| CreateNoteError::Unknown(err.into()))?;
        Ok(())
    }

    async fn list_by_owner(&self, owner: &UserId) -> Result<Vec<Note>, ListNotesError> {
        let output = self
            .client
            .query()
            .table_name(&self.table)
            .key_condition_expression("user_id = :user_id")
            .expression_attribute_values(":user_id", AttributeValue::S(owner.as_str().to_string()))
            .send()
            .await
            .map_err(|err| ListNotesError::Unknown(err.into()))?;

        output
            .items
            .unwrap_or_default()
            .into_iter()
            .map(note_from_item)
            .collect()
    }
}

fn note_from_item(item: HashMap<String, AttributeValue>) -> Result<Note, ListNotesError> {
    let owner = UserId::new(&required_s(&item, "user_id")?)
        .map_err(|err| ListNotesError::Unknown(err.into()))?;
    let id = NoteId::parse(&required_s(&item, "note_id")?)
        .map_err(|err| ListNotesError::Unknown(err.into()))?;
    let body = NoteBody::new(&required_s(&item, "body")?)
        .map_err(|err| ListNotesError::Unknown(err.into()))?;
    let created_at_ms = required_n(&item, "created_at_ms")?;
    Ok(Note::from_stored(id, owner, body, created_at_ms))
}

fn required_s(item: &HashMap<String, AttributeValue>, key: &str) -> Result<String, ListNotesError> {
    match item.get(key) {
        Some(AttributeValue::S(value)) => Ok(value.clone()),
        _ => Err(ListNotesError::Unknown(anyhow::anyhow!(
            "item is missing string attribute {key}"
        ))),
    }
}

fn required_n(item: &HashMap<String, AttributeValue>, key: &str) -> Result<i64, ListNotesError> {
    match item.get(key) {
        Some(AttributeValue::N(value)) => value.parse().map_err(|err| {
            ListNotesError::Unknown(anyhow::anyhow!("attribute {key} is not an integer: {err}"))
        }),
        _ => Err(ListNotesError::Unknown(anyhow::anyhow!(
            "item is missing number attribute {key}"
        ))),
    }
}
