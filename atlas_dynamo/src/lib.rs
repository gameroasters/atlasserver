pub mod error;

pub use crate::error::{Error, Result};
use rusoto_core::{
	credential::{DefaultCredentialsProvider, StaticProvider},
	HttpClient, Region,
};
use rusoto_dynamodb::{
	AttributeDefinition, AttributeValue, CreateTableInput, DynamoDb,
	DynamoDbClient, KeySchemaElement, ListTablesInput,
	ProvisionedThroughput,
};
use rusoto_secretsmanager::{
	GetSecretValueRequest, SecretsManager, SecretsManagerClient,
};

use std::collections::HashMap;

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
			return Err(Error::TableNotFound(table.to_string()));
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

/// should only be used for local test setups,
/// creates a DB with a compound primary key (`id`(HASH) + `sort`(RANGE))
/// # Errors
/// fails with network errors
pub async fn table_init_with_sort_key<DB>(
	db: &DB,
	table: &str,
) -> Result<()>
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
		// if !is_local_setup() {
		// 	return Err(Error::TableNotFoundError(table.to_string()));
		// }

		tracing::info!("create table: {}", table);

		let _res = db
			.create_table(CreateTableInput {
				table_name: table.into(),
				key_schema: vec![
					KeySchemaElement {
						attribute_name: "id".into(),
						key_type: "HASH".into(),
					},
					KeySchemaElement {
						attribute_name: "sort".into(),
						key_type: "RANGE".into(),
					},
				],
				attribute_definitions: vec![
					AttributeDefinition {
						attribute_name: "id".into(),
						attribute_type: "S".into(),
					},
					AttributeDefinition {
						attribute_name: "sort".into(),
						attribute_type: "S".into(),
					},
				],
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

#[must_use]
pub fn db_sort_key(
	key: &str,
	key_value: &str,
	sort: &str,
	sort_value: &str,
) -> HashMap<String, AttributeValue> {
	let mut attrs = HashMap::new();
	attrs.insert(
		key.to_string(),
		AttributeValue {
			s: Some(key_value.to_string()),
			..AttributeValue::default()
		},
	);

	attrs.insert(
		sort.to_string(),
		AttributeValue {
			s: Some(sort_value.to_string()),
			..AttributeValue::default()
		},
	);

	attrs
}

#[must_use]
fn is_local_setup() -> bool {
	std::env::var("DDB_LOCAL").is_ok()
}

pub async fn read_secret(
	secret_id: &str,
	region: Region,
) -> Result<String> {
	let manager = SecretsManagerClient::new(region);
	let val = manager
		.get_secret_value(GetSecretValueRequest {
			secret_id: secret_id.to_string(),
			..GetSecretValueRequest::default()
		})
		.await?;

	if let Some(content) = val.secret_string {
		Ok(content)
	} else {
		Err(error::Error::RusotoSecret(
			"No secret string found!".to_string(),
		))
	}
}
