use crate::error::{Error, Result};
use rusoto_core::{
	credential::{DefaultCredentialsProvider, StaticProvider},
	HttpClient, Region,
};
use rusoto_dynamodb::{
	AttributeDefinition, AttributeValue, CreateTableInput, DynamoDb,
	DynamoDbClient, KeySchemaElement, ListTablesInput,
	ProvisionedThroughput,
};
use std::collections::HashMap;

#[must_use]
pub fn db_key(
	key: &str,
	value: &str,
) -> HashMap<String, AttributeValue> {
	let mut attrs = HashMap::new();
	attrs.insert(
		key.to_string(),
		AttributeValue {
			s: Some(value.to_string()),
			..AttributeValue::default()
		},
	);
	attrs
}

pub type DynamoHashMap = HashMap<String, AttributeValue>;

/// should only be used for local test setups, creates a DB with `id`(string hash) as the primary key
/// # Errors
/// fails with network errors
pub async fn table_init<DB>(db: &DB, table: &str) -> Result<()>
where
	DB: DynamoDb + Clone + Send + Sync,
{
	let tables = db
		.list_tables(ListTablesInput {
			limit: None,
			exclusive_start_table_name: None,
		})
		.await?;

	let table_exists = tables
		.table_names
		.expect("list tables failed")
		.iter()
		.any(|n| *n == table);

	tracing::trace!("db table exists: {}", table_exists);

	if !table_exists {
		if !is_local_setup() {
			return Err(Error::TableNotFoundError(table.to_string()));
		}

		tracing::info!("create table: {}", table);

		let _res = db
			.create_table(CreateTableInput {
				table_name: table.into(),
				key_schema: vec![KeySchemaElement {
					attribute_name: "id".into(),
					key_type: "HASH".into(),
				}],
				attribute_definitions: vec![AttributeDefinition {
					attribute_name: "id".into(),
					attribute_type: "S".into(),
				}],
				provisioned_throughput: Some(ProvisionedThroughput {
					read_capacity_units: 1,
					write_capacity_units: 1,
				}),
				..CreateTableInput::default()
			})
			.await?;

		tracing::info!("table created: {:?}", table);
	}

	Ok(())
}

/// create new dynamodb connection
///
/// # Errors
///
/// http connections can fail
pub fn db_init() -> Result<DynamoDbClient> {
	let dispatcher = HttpClient::new()?;

	if is_local_setup() {
		let url = if let Ok(env) = std::env::var("DDB_URL") {
			env
		} else {
			"http://localhost:8000".into()
		};

		tracing::info!("ddb url: {}", url);

		Ok(DynamoDbClient::new_with(
			dispatcher,
			StaticProvider::new_minimal(
				"foo".to_string(),
				"bar".to_string(),
			),
			Region::Custom {
				name: "local".into(),
				endpoint: url,
			},
		))
	} else {
		Ok(DynamoDbClient::new_with(
			dispatcher,
			DefaultCredentialsProvider::new()?,
			Region::EuWest1,
		))
	}
}

fn is_local_setup() -> bool {
	std::env::var("DDB_LOCAL").is_ok()
}
