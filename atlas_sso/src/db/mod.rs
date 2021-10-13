mod dynamodb;
mod in_memory;

use crate::error::Result;
use async_trait::async_trait;
use std::collections::HashMap;
use strum::EnumString;

pub use dynamodb::DynamoSsoDB;
pub use in_memory::InMemorySsoDB;

//TODO: move into atlas
type UserId = String;

#[derive(Hash, EnumString, Debug, Eq, PartialEq, Clone, Copy)]
pub enum Provider {
	Facebook,
	SignInWithApple,
}

pub type ProviderId = String;

#[derive(Debug, PartialEq)]
pub enum SetSsoResult {
	Success,
	AlreadyAssignedDifferently,
}

#[derive(Hash, Debug, Eq, PartialEq, Clone)]
pub struct SsoKey {
	pub provider_id: ProviderId,
	pub provider: Provider,
}

impl SsoKey {
	#[must_use]
	pub fn facebook(id: &str) -> Self {
		Self {
			provider: Provider::Facebook,
			provider_id: id.to_string(),
		}
	}
}

#[derive(Debug, Clone)]
pub struct SsoEntry {
	pub user_id: UserId,
	pub provider: Provider,
	pub provider_id: ProviderId,
}

#[async_trait]
pub trait SsoDB: Send + Sync {
	async fn get_entry(&self, key: SsoKey) -> Result<SsoEntry>;
	async fn get_entries(
		&self,
		ids: &[SsoKey],
	) -> HashMap<SsoKey, SsoEntry>;

	async fn set_entry(
		&self,
		entry: SsoEntry,
	) -> Result<SetSsoResult>;

	async fn remove_entry(&self, entry: SsoEntry) -> Result<()>;
}
