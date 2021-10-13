use crate::error::Result;
use async_trait::async_trait;

#[derive(Debug, Clone)]
pub struct FcmToken {
	pub id: String,
	pub token: String,
}

#[async_trait]
pub trait FcmTokenDB: Send + Sync {
	async fn set(&self, token: FcmToken) -> Result<()>;
	async fn get(&self, user_id: &str) -> Option<FcmToken>;
}
