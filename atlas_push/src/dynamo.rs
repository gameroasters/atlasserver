use crate::{
	error::{self, Result},
	fcmtoken::{FcmToken, FcmTokenDB},
};
use async_trait::async_trait;
use atlasserver::{
	dynamo_util::{db_key, table_init, DynamoHashMap},
	error::Error,
};
use rusoto_dynamodb::{
	AttributeValue, DynamoDb, DynamoDbClient, GetItemInput,
	PutItemInput,
};
use std::{
	collections::HashMap,
	convert::{TryFrom, TryInto},
};
use tracing::instrument;

#[derive(Clone)]
pub struct DynamoFcmTokenDB {
	db: DynamoDbClient,
	table: String,
}

impl DynamoFcmTokenDB {
	/// # Errors
	/// Returns an error if the table is not initiated
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

	#[instrument(skip(self))]
	async fn load(&self, key: &str) -> Option<FcmToken> {
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

	#[instrument(skip(self), err)]
	async fn save(&self, token: FcmToken) -> Result<()> {
		let input = PutItemInput {
			table_name: self.table.clone(),
			item: token.into(),
			..PutItemInput::default()
		};

		self.db.put_item(input).await?;

		tracing::debug!("saved");

		Ok(())
	}
}

impl From<FcmToken> for DynamoHashMap {
	fn from(v: FcmToken) -> Self {
		let mut map = Self::with_capacity(2);
		map.insert(
			"id".to_string(),
			AttributeValue {
				s: Some(v.id),
				..AttributeValue::default()
			},
		);
		map.insert(
			"token".to_string(),
			AttributeValue {
				s: Some(v.token),
				..AttributeValue::default()
			},
		);
		map
	}
}

impl TryFrom<HashMap<String, AttributeValue>> for FcmToken {
	type Error = error::Error;

	fn try_from(
		map: HashMap<String, AttributeValue>,
	) -> Result<Self> {
		Ok(Self {
			id: map
				.get("id")
				.and_then(|attr| attr.s.clone())
				.ok_or(Error::DynamoDeserialize("id"))?,
			token: map
				.get("token")
				.and_then(|attr| attr.s.clone())
				.ok_or(Error::DynamoDeserialize("token"))?,
		})
	}
}

#[async_trait]
impl FcmTokenDB for DynamoFcmTokenDB {
	async fn set(&self, token: FcmToken) -> Result<()> {
		Ok(self.save(token).await?)
	}

	async fn get(&self, user_id: &str) -> Option<FcmToken> {
		self.load(user_id).await
	}
}
