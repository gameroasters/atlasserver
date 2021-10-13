use super::{Platform, ReceiptDB};
use crate::{error::Result, Receipt};
use async_trait::async_trait;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::Mutex;

#[derive(Default)]
pub struct InMemoryReceiptDB {
	db: Arc<Mutex<HashMap<(String, String), Receipt>>>,
}

#[async_trait]
impl ReceiptDB for InMemoryReceiptDB {
	async fn save_receipt(&self, r: Receipt) -> Result<()> {
		let mut db = self.db.lock().await;
		db.insert(
			(r.transaction_id.clone(), r.platform.to_string()),
			r,
		);
		Ok(())
	}

	async fn get_receipt(
		&self,
		transaction_id: &str,
		platform: Platform,
	) -> Option<Receipt> {
		let db = self.db.lock().await;
		db.get(&(transaction_id.to_string(), platform.to_string()))
			.cloned()
	}
}
