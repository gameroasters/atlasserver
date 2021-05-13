use super::{Session, SessionDB};
use crate::error::Result;
use async_trait::async_trait;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::Mutex;

#[derive(Default)]
pub struct InMemorySessionDB {
	pub db: Arc<Mutex<HashMap<String, Session>>>,
}

#[async_trait]
impl SessionDB for InMemorySessionDB {
	async fn get(&self, key: &str) -> Option<Session> {
		self.db.lock().await.get(&key.to_string()).cloned()
	}

	async fn invalidate(&self, key: &str) -> Option<()> {
		self.db.lock().await.get_mut(&key.to_string()).map(
			|session| {
				session.valid = false;
			},
		)
	}

	async fn create(&self, session: Session) -> Result<String> {
		let key = Session::new_key();

		self.db.lock().await.insert(key.clone(), session);

		Ok(key)
	}
}
