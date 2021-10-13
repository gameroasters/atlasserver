use std::{collections::HashMap, sync::Arc};

use crate::{error::Result, fcmtoken::FcmToken};
use async_trait::async_trait;
use tokio::sync::Mutex;

use crate::fcmtoken::FcmTokenDB;

#[derive(Default)]
pub struct InMemoryFcmTokenDB {
	pub db: Arc<Mutex<HashMap<String, FcmToken>>>,
}

#[async_trait]
impl FcmTokenDB for InMemoryFcmTokenDB {
	async fn set(&self, token: FcmToken) -> Result<()> {
		let mut db = self.db.lock().await;

		db.insert(token.id.clone(), token);

		Ok(())
	}

	async fn get(&self, user_id: &str) -> Option<FcmToken> {
		let db = self.db.lock().await;
		db.get(&user_id.to_string()).cloned()
	}
}
