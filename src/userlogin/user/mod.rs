pub mod dynamodb;
pub mod in_memory;

use crate::error::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(
	Default, Clone, Debug, PartialEq, Serialize, Deserialize,
)]
pub struct User {
	pub id: String,
	pub secret: String,
	pub version: u64,
	pub session: Option<String>,
	pub country: Option<String>,
	pub language: Option<String>,
}

impl User {
	#[must_use]
	pub fn new(
		country: Option<String>,
		language: Option<String>,
	) -> Self {
		Self {
			id: Uuid::new_v4().to_string(),
			secret: Uuid::new_v4().to_string(),
			country,
			language,
			..Self::default()
		}
	}
}

#[async_trait]
pub trait UserDB: Send + Sync {
	async fn get_user(&self, key: &str) -> Option<User>;
	async fn save_user(&self, u: &User) -> Result<()>;
}
