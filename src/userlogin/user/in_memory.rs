use super::{User, UserDB};
use crate::error::Result;
use async_trait::async_trait;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::Mutex;

#[derive(Default)]
pub struct InMemoryUserDB {
	pub db: Arc<Mutex<HashMap<String, User>>>,
}

#[async_trait]
impl UserDB for InMemoryUserDB {
	async fn get_user(&self, key: &str) -> Option<User> {
		let db = self.db.lock().await;
		db.get(&key.to_string()).cloned()
	}

	async fn save_user(&self, u: &User) -> Result<()> {
		let db = &mut self.db.lock().await;
		db.insert(u.id.clone(), u.clone());

		Ok(())
	}
}
