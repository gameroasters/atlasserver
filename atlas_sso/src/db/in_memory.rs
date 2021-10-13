use super::{SetSsoResult, SsoDB, SsoEntry, SsoKey};
use crate::error::{Error, Result};
use async_trait::async_trait;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::Mutex;
use tracing::instrument;

#[derive(Default)]
pub struct InMemorySsoDB {
	pub db: Arc<Mutex<HashMap<SsoKey, SsoEntry>>>,
}

#[async_trait]
impl SsoDB for InMemorySsoDB {
	#[instrument(skip(self), err)]
	async fn get_entry(&self, key: SsoKey) -> Result<SsoEntry> {
		let db = self.db.lock().await;

		db.get(&key).cloned().ok_or_else(|| {
			Error::Generic(String::from("in memory get failed"))
		})
	}

	#[instrument(skip(self), err)]
	async fn set_entry(
		&self,
		entry: SsoEntry,
	) -> Result<SetSsoResult> {
		let mut db = self.db.lock().await;
		let key = SsoKey {
			provider: entry.provider,
			provider_id: entry.provider_id.clone(),
		};
		if db
			.get(&key)
			.map(|e| e.user_id != entry.user_id)
			.unwrap_or_default()
		{
			return Ok(SetSsoResult::AlreadyAssignedDifferently);
		}
		db.insert(key, entry);

		return Ok(SetSsoResult::Success);
	}

	async fn get_entries(
		&self,
		_ids: &[SsoKey],
	) -> HashMap<SsoKey, SsoEntry> {
		tracing::error!("not implemented");
		HashMap::new()
	}

	async fn remove_entry(&self, _entry: SsoEntry) -> Result<()> {
		tracing::error!("not implemented");
		Err(Error::Generic(String::from("not implemented")))
	}
}

#[cfg(test)]
mod tests {
	#![allow(
		clippy::unwrap_used,
        //TODO: https://github.com/rust-lang/rust-clippy/issues/7438
		clippy::semicolon_if_nothing_returned
	)]
	use super::*;
	use crate::Provider;
	use pretty_assertions::assert_eq;

	#[tokio::test]
	async fn test_set_entry() {
		let db = InMemorySsoDB::default();

		let u = String::from("u1");
		let u2 = String::from("u2");
		let p = String::from("p");

		let r = db
			.set_entry(SsoEntry {
				user_id: u.clone(),
				provider_id: p.clone(),
				provider: Provider::Facebook,
			})
			.await
			.unwrap();

		assert_eq!(r, SetSsoResult::Success);

		let r = db
			.set_entry(SsoEntry {
				user_id: u.clone(),
				provider_id: p.clone(),
				provider: Provider::Facebook,
			})
			.await
			.unwrap();

		assert_eq!(r, SetSsoResult::Success);

		let r = db
			.set_entry(SsoEntry {
				user_id: u2.clone(),
				provider_id: p.clone(),
				provider: Provider::Facebook,
			})
			.await
			.unwrap();

		assert_eq!(r, SetSsoResult::AlreadyAssignedDifferently);
	}
}
