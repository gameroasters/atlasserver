use std::{
	collections::HashMap,
	convert::{TryFrom, TryInto},
};

use super::{User, UserDB};
use crate::{
	dynamo_util::{db_key, table_init, DynamoHashMap},
	error::{Error, Result},
};
use async_trait::async_trait;
use rusoto_dynamodb::{
	AttributeValue, DynamoDb, DynamoDbClient, GetItemInput,
	PutItemInput,
};

#[derive(Clone)]
pub struct DynamoUserDB {
	db: DynamoDbClient,
	table: String,
}

impl DynamoUserDB {
	/// create new `DynamoUserDB` instance reusing an existing db client connection
	///
	/// # Errors
	///
	/// local table init could fail creating table of the check
	/// for the existance of the right table remote coul fail
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

	async fn load(&self, key: &str) -> Option<User> {
		let item = self
			.db
			.get_item(GetItemInput {
				table_name: self.table.clone(),
				key: db_key("id", key),
				..GetItemInput::default()
			})
			.await
			.ok()?
			.item?;

		item.try_into().ok()
	}

	async fn save(&self, user: User) -> Result<()> {
		let item_version = user.version;
		let mut input = PutItemInput {
			table_name: self.table.clone(),
			item: user.into(),
			..PutItemInput::default()
		};

		if item_version > 0 {
			let mut value_map = HashMap::new();
			value_map.insert(
				":ver".to_string(),
				AttributeValue {
					n: Some(format!("{}", item_version - 1)),
					..AttributeValue::default()
				},
			);

			input.condition_expression =
				Some("version = :ver".into());
			input.expression_attribute_values = Some(value_map);
		}

		self.db.put_item(input).await?;

		Ok(())
	}
}

impl From<User> for DynamoHashMap {
	fn from(v: User) -> Self {
		let mut map = Self::new();
		map.insert(
			"id".to_string(),
			AttributeValue {
				s: Some(v.id),
				..AttributeValue::default()
			},
		);
		map.insert(
			"version".to_string(),
			AttributeValue {
				n: Some(v.version.to_string()),
				..AttributeValue::default()
			},
		);
		map.insert(
			"secret".to_string(),
			AttributeValue {
				s: Some(v.secret),
				..AttributeValue::default()
			},
		);
		if let Some(session) = v.session {
			map.insert(
				"session".to_string(),
				AttributeValue {
					s: Some(session),
					..AttributeValue::default()
				},
			);
		};
		if let Some(country) = v.country {
			map.insert(
				"country".to_string(),
				AttributeValue {
					s: Some(country),
					..AttributeValue::default()
				},
			);
		};
		if let Some(language) = v.language {
			map.insert(
				"language".to_string(),
				AttributeValue {
					s: Some(language),
					..AttributeValue::default()
				},
			);
		};

		map
	}
}

impl TryFrom<HashMap<String, AttributeValue>> for User {
	type Error = crate::error::Error;
	fn try_from(
		attributes: HashMap<String, AttributeValue>,
	) -> Result<Self> {
		Ok(Self {
			id: attributes
				.get("id")
				.and_then(|attr| attr.s.clone())
				.ok_or(Error::DynamoDeserializeError("id"))?,
			version: attributes
				.get("version")
				.and_then(|attr| attr.n.as_ref())
				.and_then(|n| n.parse::<u64>().ok())
				.ok_or(Error::DynamoDeserializeError("version"))?,
			secret: attributes
				.get("secret")
				.and_then(|attr| attr.s.clone())
				.ok_or(Error::DynamoDeserializeError("secret"))?,
			session: attributes
				.get("session")
				.and_then(|attr| attr.s.clone()),
			country: attributes
				.get("country")
				.and_then(|attr| attr.s.clone()),
			language: attributes
				.get("language")
				.and_then(|attr| attr.s.clone()),
		})
	}
}

#[async_trait]
impl UserDB for DynamoUserDB {
	async fn get_user(&self, key: &str) -> Option<User> {
		self.load(key).await
	}

	//TODO: take user as value
	//TODO: return well defined error types so client know whether to retry because of version mismatch
	async fn save_user(&self, u: &User) -> Result<()> {
		Ok(self.save(u.clone()).await?)
	}
}
