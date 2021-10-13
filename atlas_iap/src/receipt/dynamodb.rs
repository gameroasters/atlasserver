use super::ReceiptDB;
use crate::{
	error::{Error, Result},
	utc_time::UtcDateTime,
	Platform, Receipt,
};
use async_trait::async_trait;
use atlas_dynamo::{db_sort_key, table_init_with_sort_key};
use atlasserver::dynamo_util::DynamoHashMap;
use rusoto_dynamodb::{
	AttributeValue, DynamoDb, DynamoDbClient, GetItemInput,
	PutItemInput,
};
use std::{
	convert::{TryFrom, TryInto},
	str::FromStr,
};

pub struct DynamoReceiptDB {
	db: DynamoDbClient,
	table: String,
}

impl DynamoReceiptDB {
	pub async fn new(
		table_name: &str,
		db: DynamoDbClient,
	) -> Result<Self> {
		table_init_with_sort_key(&db, table_name).await?;
		Ok(Self {
			db,
			table: table_name.to_string(),
		})
	}

	async fn save(&self, r: Receipt) -> Result<()> {
		let input = PutItemInput {
			table_name: self.table.clone(),
			item: r.clone().into(),
			..PutItemInput::default()
		};

		self.db.put_item(input).await?;

		Ok(())
	}

	async fn load(
		&self,
		transaction_id: &str,
		platform: Platform,
	) -> Option<Receipt> {
		let item = self
			.db
			.get_item(GetItemInput {
				table_name: self.table.clone(),
				key: db_sort_key(
					"id",
					transaction_id,
					"sort",
					&platform.to_string(),
				),
				..GetItemInput::default()
			})
			.await
			.ok()?
			.item?;

		item.try_into().ok()
	}
}

#[async_trait]
impl ReceiptDB for DynamoReceiptDB {
	async fn save_receipt(&self, r: crate::Receipt) -> Result<()> {
		self.save(r).await
	}

	async fn get_receipt(
		&self,
		transaction_id: &str,
		platform: Platform,
	) -> Option<crate::Receipt> {
		self.load(transaction_id, platform).await
	}
}

impl From<Receipt> for DynamoHashMap {
	fn from(v: Receipt) -> Self {
		let mut map = Self::new();

		map.insert(
			"id".to_string(),
			AttributeValue {
				s: Some(v.transaction_id.clone()),
				..AttributeValue::default()
			},
		);

		map.insert(
			"sort".to_string(),
			AttributeValue {
				s: Some(v.platform.to_string()),
				..AttributeValue::default()
			},
		);

		map.insert(
			"unity_receipt".to_string(),
			AttributeValue {
				s: Some(v.unity_receipt),
				..AttributeValue::default()
			},
		);

		map.insert(
			"user_id".to_string(),
			AttributeValue {
				s: Some(v.user_id.clone()),
				..AttributeValue::default()
			},
		);

		map.insert(
			"product_id".to_string(),
			AttributeValue {
				s: Some(v.product_id.clone()),
				..AttributeValue::default()
			},
		);

		map.insert(
			"is_subscription".to_string(),
			AttributeValue {
				bool: Some(v.is_subscription),
				..AttributeValue::default()
			},
		);

		map.insert(
			"date".to_string(),
			AttributeValue {
				s: Some(v.date.to_string()),
				..AttributeValue::default()
			},
		);

		map
	}
}

impl TryFrom<DynamoHashMap> for Receipt {
	type Error = Error;

	fn try_from(attributes: DynamoHashMap) -> Result<Self> {
		Ok(Self {
			transaction_id: attributes
				.get("id")
				.and_then(|attr| attr.s.clone())
				.ok_or(Error::DynamoDeserialize("id"))?,
			platform: attributes
				.get("sort")
				.and_then(|attr| attr.s.as_deref())
				.and_then(|s| Platform::from_str(s).ok())
				.ok_or(Error::DynamoDeserialize("sort"))?,
			unity_receipt: attributes
				.get("unity_receipt")
				.and_then(|attr| attr.s.clone())
				.ok_or(Error::DynamoDeserialize("unity_receipt"))?,
			user_id: attributes
				.get("user_id")
				.and_then(|attr| attr.s.clone())
				.ok_or(Error::DynamoDeserialize("user_id"))?,
			is_subscription: attributes
				.get("is_subscription")
				.and_then(|attr| attr.bool)
				.ok_or(Error::DynamoDeserialize("is_subscription"))?,
			product_id: attributes
				.get("product_id")
				.and_then(|attr| attr.s.clone())
				.ok_or(Error::DynamoDeserialize("product_id"))?,
			date: attributes
				.get("date")
				.and_then(|attr| attr.s.as_ref())
				.and_then(|s| s.parse::<UtcDateTime>().ok())
				.ok_or(Error::DynamoDeserialize("date"))?,
		})
	}
}
