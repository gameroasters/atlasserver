use super::{Session, SessionDB};
use crate::{
	dynamo_util::{db_key, table_init, DynamoHashMap},
	error::{Error, Result},
};
use async_trait::async_trait;
use chrono::{DateTime, Duration, NaiveDateTime, Utc};
use rusoto_dynamodb::{
	AttributeValue, DynamoDb, DynamoDbClient, PutItemInput,
	UpdateItemInput,
};
use std::{
	collections::HashMap,
	convert::{TryFrom, TryInto},
};
use tracing::instrument;

#[derive(Debug, PartialEq, Clone)]
struct DynamoSession {
	id: String,
	user_id: String,
	valid: bool,
	ttl: i64,
}

impl DynamoSession {
	#[must_use]
	fn new(session: Session, ttl: i64) -> Self {
		let id = Session::new_key();
		Self {
			user_id: session.user_id,
			valid: session.valid,
			id,
			ttl,
		}
	}
}

impl From<DynamoSession> for Session {
	fn from(session: DynamoSession) -> Self {
		Self {
			user_id: session.user_id,
			valid: session.valid,
		}
	}
}

#[derive(Clone)]
pub struct DynamoSessionDB {
	db: DynamoDbClient,
	table: String,
}

impl DynamoSessionDB {
	/// create new `DynamoUserDB` instance reusing an existing db client connection
	///
	/// # Errors
	///
	/// local table init could fail creating table of the check
	/// for the existance of the right table remote could fail
	pub async fn new(
		table_name: &str,
		db: DynamoDbClient,
	) -> Result<Self> {
		table_init(&db, table_name).await?;
		Ok(Self {
			db,
			table: table_name.to_string(),
		})
	}

	#[must_use]
	pub fn ttl(now: DateTime<Utc>) -> i64 {
		let now: DateTime<Utc> = now + Duration::minutes(5);
		now.timestamp()
	}
}

impl From<DynamoSession> for DynamoHashMap {
	fn from(session: DynamoSession) -> Self {
		let mut map = Self::new();
		map.insert(
			"id".to_string(),
			AttributeValue {
				s: Some(session.id),
				..AttributeValue::default()
			},
		);
		map.insert(
			"user_id".to_string(),
			AttributeValue {
				s: Some(session.user_id),
				..AttributeValue::default()
			},
		);
		map.insert(
			"valid".to_string(),
			AttributeValue {
				s: Some(session.valid.to_string()),
				..AttributeValue::default()
			},
		);
		map.insert(
			"ttl".to_string(),
			AttributeValue {
				n: Some(session.ttl.to_string()),
				..AttributeValue::default()
			},
		);

		map
	}
}

impl TryFrom<DynamoHashMap> for DynamoSession {
	type Error = crate::error::Error;

	fn try_from(attributes: DynamoHashMap) -> Result<Self> {
		Ok(Self {
			id: attributes
				.get(&"id".to_string())
				.and_then(|attr| attr.s.clone())
				.ok_or(Error::DynamoDeserializeError("id"))?,
			user_id: attributes
				.get(&"user_id".to_string())
				.and_then(|attr| attr.s.clone())
				.ok_or(Error::DynamoDeserializeError("user_id"))?,
			valid: attributes
				.get(&"valid".to_string())
				.and_then(|attr| attr.s.as_ref())
				.and_then(|attr| attr.parse::<bool>().ok())
				.ok_or(Error::DynamoDeserializeError("valid"))?,
			ttl: attributes
				.get(&"ttl".to_string())
				.and_then(|attr| attr.n.as_ref())
				.and_then(|attr| attr.parse::<i64>().ok())
				.ok_or(Error::DynamoDeserializeError("ttl"))?,
		})
	}
}

#[async_trait]
impl SessionDB for DynamoSessionDB {
	#[instrument(skip(self), err)]
	async fn create(&self, session: Session) -> Result<String> {
		tracing::trace!("SessionDB::create");

		let session =
			DynamoSession::new(session, Self::ttl(Utc::now()));
		let key = session.id.clone();

		let mut input = PutItemInput {
			table_name: self.table.clone(),
			item: session.into(),
			..PutItemInput::default()
		};

		input.condition_expression =
			Some("attribute_not_exists(id)".into());

		self.db.put_item(input).await?;

		Ok(key)
	}

	#[instrument(skip(self))]
	async fn invalidate(&self, key: &str) -> Option<()> {
		tracing::debug!("SessionDB::invalidate");

		let mut value_map = HashMap::new();
		value_map.insert(
			":val".to_string(),
			AttributeValue {
				s: Some(String::from("false")),
				..AttributeValue::default()
			},
		);

		let input = UpdateItemInput {
			table_name: self.table.clone(),
			key: db_key("id", key),
			update_expression: Some(String::from("SET valid = :val")),
			condition_expression: Some(String::from(
				"attribute_exists(id)",
			)),
			expression_attribute_values: Some(value_map),
			..UpdateItemInput::default()
		};

		if let Err(e) = self.db.update_item(input).await {
			tracing::error!("error invalidating session: {}", e);
			None
		} else {
			Some(())
		}
	}

	#[instrument(skip(self))]
	async fn get(&self, key: &str) -> Option<Session> {
		tracing::trace!("SessionDB::get");

		let new_ttl = Self::ttl(Utc::now());

		let mut value_map = HashMap::new();
		value_map.insert(
			":ttl".to_string(),
			AttributeValue {
				n: Some(new_ttl.to_string()),
				..AttributeValue::default()
			},
		);

		let mut name_map = HashMap::new();
		name_map.insert("#ttl".to_string(), "ttl".to_string());

		let input = UpdateItemInput {
			table_name: self.table.clone(),
			key: db_key("id", key),
			condition_expression: Some(String::from(
				"attribute_exists(id)",
			)),
			update_expression: Some(String::from("SET #ttl = :ttl")),
			return_values: Some(String::from("ALL_NEW")),
			expression_attribute_values: Some(value_map),
			expression_attribute_names: Some(name_map),
			..UpdateItemInput::default()
		};

		let item: DynamoSession = self
			.db
			.update_item(input)
			.await
			.map_err(|e| tracing::error!("update error: {}", e))
			.ok()?
			.attributes?
			.try_into()
			.map_err(|e| tracing::error!("try_into error: {}", e))
			.ok()?;

		let ttl = DateTime::<Utc>::from_utc(
			NaiveDateTime::from_timestamp(item.ttl, 0),
			Utc,
		);
		if ttl < Utc::now() {
			tracing::error!("session timeout");
			return None;
		}

		Some(item.into())
	}
}

#[cfg(test)]
mod test {
	use super::*;
	use pretty_assertions::assert_eq;

	#[test]
	fn test_serialize() {
		let s = DynamoSession {
			id: String::from("sid"),
			user_id: String::from("uid"),
			valid: false,
			ttl: 0,
		};

		let map: DynamoHashMap = s.clone().try_into().unwrap();

		let s2: DynamoSession = dbg!(map).try_into().unwrap();

		assert_eq!(s, s2);
	}
}

#[cfg(test)]
mod test_ddb {
	use super::*;
	use json::{object, JsonValue};
	use mockito::mock;
	use rusoto_core::{
		credential::StaticProvider, HttpClient, Region,
	};

	#[tokio::test]
	async fn test_session_not_existent() {
		let (db, _) = create_test_ddb_session().await;

		let nonexist_res = db.get("invalid").await;

		assert!(nonexist_res.is_none());
	}

	#[tokio::test]
	async fn test_session_create() {
		let (db, _) = create_test_ddb_session().await;

		let mock =
			mock_ddb_request_ok("PutItem", object! {}).expect(1);

		let session =
			db.create(Session::new("test-user-id")).await.unwrap();

		mock.assert();

		assert!(!session.is_empty());
	}

	#[tokio::test]
	async fn test_session_valid() {
		let (db, _) = create_test_ddb_session().await;

		let mock = mock_ddb_request_ok(
			"UpdateItem",
			object! {
				Attributes: {
					id: {S: "session"},
					user_id: {S: "uid"},
					valid: {S: "true"},
					//TODO: unhardcode timestamp
					ttl: {N: "10000000000"},
				}
			},
		)
		.expect(1);

		let valid_res = db.get("sessionid").await;

		mock.assert();

		assert!(valid_res.is_some());
		assert!(valid_res.as_ref().unwrap().valid);
		assert_eq!(valid_res.unwrap().user_id, "uid");
	}

	#[tokio::test]
	async fn test_session_invalid() {
		let (db, _) = create_test_ddb_session().await;

		let mock = mock_ddb_request_ok(
			"UpdateItem",
			object! {
				Attributes: {
					id: {S: "session"},
					user_id: {S: "uid"},
					valid: {S: "false"},
					//TODO: unhardcode timestamp
					ttl: {N: "10000000000"},
				}
			},
		)
		.expect(1);

		let valid_res = db.get("sessionid").await;

		mock.assert();

		assert!(valid_res.is_some());
		assert!(!valid_res.as_ref().unwrap().valid);
	}

	#[tokio::test]
	async fn test_session_invalidate() {
		let (db, _) = create_test_ddb_session().await;

		let mock =
			mock_ddb_request_ok("UpdateItem", object! {}).expect(1);

		let res = db.invalidate("").await;

		mock.assert();

		assert!(res.is_some());
	}

	async fn create_test_ddb_session(
	) -> (DynamoSessionDB, mockito::Mock) {
		// enable env logger
		let _ = env_logger::try_init();

		let table_name = "table";
		let data = object! {
			LastEvaluatedTableName: "string",
			TableNames: [table_name]
		};

		// DynamoSessionDB::new will call `ListTables`
		let mock = mock_ddb_request_ok("ListTables", data);
		let db = DynamoDbClient::new_with(
			HttpClient::new().unwrap(),
			StaticProvider::new_minimal(
				"foo".to_string(),
				"bar".to_string(),
			),
			Region::Custom {
				name: "local".into(),
				endpoint: mockito::server_url(),
			},
		);

		let db = DynamoSessionDB::new(table_name, db).await.unwrap();
		(db, mock)
	}

	#[tokio::test]
	async fn test_session_invalidate_fail() {
		let (db, _) = create_test_ddb_session().await;

		let mock =
			mock_ddb_request("UpdateItem", object! {}, 501).expect(1);

		let res = db.invalidate("").await;

		mock.assert();

		assert!(res.is_none());
	}

	fn mock_ddb_request_ok(
		endpoint: &str,
		res: JsonValue,
	) -> mockito::Mock {
		mock_ddb_request(endpoint, res, 200)
	}

	fn mock_ddb_request(
		endpoint: &str,
		res: JsonValue,
		status: usize,
	) -> mockito::Mock {
		mock("POST", "/")
			.with_status(status)
			.with_header(
				"x-amz-target",
				format!("DynamoDB_20120810.{}", endpoint).as_str(),
			)
			.with_body(res.dump())
			.create()
	}
}
