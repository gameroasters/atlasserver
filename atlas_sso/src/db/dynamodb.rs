use crate::{
	dynamo_table::{db_sort_key, table_init_with_sort_key},
	error::{Error, Result},
	Provider, SetSsoResult, SsoDB, SsoEntry, SsoKey,
};
use async_trait::async_trait;
use atlasserver::{dynamo_util::DynamoHashMap, error};
use rusoto_dynamodb::{
	AttributeValue, BatchGetItemInput, DeleteItemInput, DynamoDb,
	DynamoDbClient, GetItemInput, KeysAndAttributes, PutItemInput,
};
use std::{
	collections::HashMap,
	convert::{TryFrom, TryInto},
	str::FromStr,
};
use tracing::instrument;

pub struct DynamoSsoDB {
	db: DynamoDbClient,
	table: String,
}

impl DynamoSsoDB {
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

	async fn load(&self, key: SsoKey) -> Option<SsoEntry> {
		let item = self
			.db
			.get_item(GetItemInput {
				table_name: self.table.clone(),
				key: db_sort_key(
					"id",
					&key.provider_id,
					"sort",
					&format!("{:?}", key.provider),
				),
				..GetItemInput::default()
			})
			.await
			.ok()?
			.item?;

		item.try_into().ok()
	}

	async fn remove(&self, key: SsoKey) -> Result<()> {
		self.db
			.delete_item(DeleteItemInput {
				table_name: self.table.clone(),
				key: db_sort_key(
					"id",
					&key.provider_id,
					"sort",
					&format!("{:?}", key.provider),
				),
				..DeleteItemInput::default()
			})
			.await?;

		Ok(())
	}

	async fn load_batch(
		&self,
		key: &[SsoKey],
	) -> Option<HashMap<SsoKey, SsoEntry>> {
		let keys = key
			.iter()
			.map(|key| {
				db_sort_key(
					"id",
					&key.provider_id,
					"sort",
					&format!("{:?}", key.provider),
				)
			})
			.collect::<Vec<_>>();

		let keys = KeysAndAttributes {
			keys,
			..KeysAndAttributes::default()
		};

		let mut request_items: HashMap<String, KeysAndAttributes> =
			HashMap::with_capacity(1);
		request_items.insert(self.table.clone(), keys);

		let res = self
			.db
			.batch_get_item(BatchGetItemInput {
				request_items,
				..BatchGetItemInput::default()
			})
			.await
			.ok()?;

		if let Some(keys) = res.unprocessed_keys {
			tracing::warn!("batch missing keys: {}", keys.len());
		}

		res.responses
			.and_then(|responses| responses.into_iter().next())
			.map(|(_table, elements)| {
				elements
					.into_iter()
					.filter_map(|value| {
						SsoEntry::try_from(value).ok().map(|entry| {
							(
								SsoKey {
									provider: entry.provider,
									provider_id: entry
										.provider_id
										.clone(),
								},
								entry,
							)
						})
					})
					.collect()
			})
	}

	async fn save(&self, entry: SsoEntry) -> Result<SetSsoResult> {
		if self
			.load(SsoKey {
				provider: entry.provider,
				provider_id: entry.provider_id.clone(),
			})
			.await
			.as_ref()
			.map(|e| e.user_id != entry.user_id)
			.unwrap_or_default()
		{
			return Ok(SetSsoResult::AlreadyAssignedDifferently);
		}

		let input = PutItemInput {
			table_name: self.table.clone(),
			item: entry.clone().into(),
			..PutItemInput::default()
		};

		self.db.put_item(input).await?;

		Ok(SetSsoResult::Success)
	}
}

#[async_trait]
impl SsoDB for DynamoSsoDB {
	#[instrument(skip(self), err)]
	async fn get_entry(&self, key: SsoKey) -> Result<SsoEntry> {
		self.load(key).await.ok_or(Error::GetEntry)
	}

	#[instrument(skip(self), err)]
	async fn set_entry(
		&self,
		entry: SsoEntry,
	) -> Result<SetSsoResult> {
		self.save(entry).await
	}

	#[instrument(skip(self, ids))]
	async fn get_entries(
		&self,
		ids: &[SsoKey],
	) -> HashMap<SsoKey, SsoEntry> {
		tracing::info!(target: "get_entries",count = %ids.len());

		self.load_batch(ids).await.unwrap_or_default()
	}

	#[instrument(skip(self), err)]
	async fn remove_entry(&self, entry: SsoEntry) -> Result<()> {
		self.remove(SsoKey {
			provider: entry.provider,
			provider_id: entry.provider_id,
		})
		.await?;

		Ok(())
	}
}

impl From<SsoEntry> for DynamoHashMap {
	fn from(v: SsoEntry) -> Self {
		let mut map = Self::new();

		map.insert(
			"user_id".to_string(),
			AttributeValue {
				s: Some(v.user_id.clone()),
				..AttributeValue::default()
			},
		);

		map.insert(
			"sort".to_string(),
			AttributeValue {
				s: Some(format!("{:?}", v.provider)),
				..AttributeValue::default()
			},
		);

		map.insert(
			"id".to_string(),
			AttributeValue {
				s: Some(v.provider_id),
				..AttributeValue::default()
			},
		);

		map
	}
}

impl TryFrom<DynamoHashMap> for SsoEntry {
	type Error = crate::error::Error;

	fn try_from(attributes: DynamoHashMap) -> Result<Self> {
		Ok(Self {
			user_id: attributes
				.get("user_id")
				.and_then(|attr| attr.s.clone())
				.ok_or(error::Error::DynamoDeserialize("user_id"))?,
			provider: attributes
				.get("sort")
				.and_then(|attr| attr.s.as_ref())
				.and_then(|s| Provider::from_str(s).ok())
				.ok_or(error::Error::DynamoDeserialize("sort"))?,
			provider_id: attributes
				.get("id")
				.and_then(|attr| attr.s.clone())
				.ok_or(error::Error::DynamoDeserialize("id"))?,
		})
	}
}
