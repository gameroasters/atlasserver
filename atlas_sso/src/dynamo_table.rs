//TODO: move into its own crate atlas_dynamo

use crate::error::Result;
use rusoto_dynamodb::{
	AttributeDefinition, AttributeValue, CreateTableInput, DynamoDb,
	KeySchemaElement, ListTablesInput, ProvisionedThroughput,
};
use std::collections::HashMap;

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
