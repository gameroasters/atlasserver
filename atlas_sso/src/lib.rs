#![forbid(unsafe_code)]
#![deny(
	dead_code,
	unused_imports,
	unused_must_use,
	unused_variables,
	unused_mut
)]
#![deny(clippy::all, clippy::pedantic, clippy::nursery)]
#![deny(
	clippy::as_conversions,
	clippy::dbg_macro,
	clippy::float_cmp_const,
	clippy::lossy_float_literal,
	clippy::string_to_string,
	clippy::unneeded_field_pattern,
	clippy::verbose_file_reads,
	clippy::unwrap_used,
	clippy::panic,
	clippy::needless_update,
	clippy::match_like_matches_macro,
	clippy::from_over_into,
	clippy::useless_conversion
)]
#![allow(clippy::module_name_repetitions)]
//TODO:
#![allow(clippy::missing_errors_doc)]

mod db;
pub mod error;
pub mod schema;
mod siwa;

pub use self::{
	db::{
		DynamoSsoDB, InMemorySsoDB, Provider, SetSsoResult, SsoDB,
		SsoEntry, SsoKey,
	},
	siwa::create_filters_siwa,
};
use atlasserver::userlogin::user::{User, UserDB};
use error::{Error, Result};
use std::{collections::HashMap, sync::Arc};
use tracing::instrument;

pub struct SsoResource {
	sso_db: Arc<dyn SsoDB>,
	users: Arc<dyn UserDB>,
}

impl SsoResource {
	#[must_use]
	pub fn new(
		sso_db: Arc<dyn SsoDB>,
		users: Arc<dyn UserDB>,
	) -> Self {
		Self { sso_db, users }
	}

	pub async fn set_sso(
		&self,
		sso: SsoEntry,
	) -> Result<SetSsoResult> {
		self.sso_db.set_entry(sso).await
	}

	pub async fn remove_entry(&self, entry: SsoEntry) -> Result<()> {
		self.sso_db.remove_entry(entry).await
	}

	pub async fn get_user(&self, key: SsoKey) -> Result<User> {
		let sso = self.sso_db.get_entry(key).await?;

		if let Some(user) = self.users.get_user(&sso.user_id).await {
			return Ok(user);
		}

		Err(Error::UnknownUser)
	}

	#[instrument(skip(self, keys))]
	pub async fn get_user_ids(
		&self,
		keys: &[SsoKey],
	) -> HashMap<SsoKey, SsoEntry> {
		tracing::info!(target: "get_user_ids", count = %keys.len());

		let mut res = HashMap::with_capacity(keys.len());
		for chunk in keys.chunks(100) {
			let chunk_result = self.sso_db.get_entries(chunk).await;
			res.extend(chunk_result);
		}
		res
	}
}
