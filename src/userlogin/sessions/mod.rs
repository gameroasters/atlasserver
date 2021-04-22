mod dynamodb;
mod in_memory;

pub use dynamodb::DynamoSessionDB;
pub use in_memory::InMemorySessionDB;

use crate::error::Result;
use async_trait::async_trait;

#[derive(Debug, Clone)]
pub struct Session {
	pub user_id: String,
	pub valid: bool,
}

impl Session {
	#[must_use]
	pub fn new(user_id: &str) -> Self {
		Self {
			user_id: user_id.to_string(),
			valid: true,
		}
	}

	fn new_key() -> String {
		uuid::Uuid::new_v4().to_string()
	}
}

#[async_trait]
pub trait SessionDB: Send + Sync {
	async fn create(&self, session: Session) -> Result<String>;
	async fn invalidate(&self, key: &str);
	async fn get(&self, key: &str) -> Option<Session>;
}
