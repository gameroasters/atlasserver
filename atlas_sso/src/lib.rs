//! An atlas module used for storing Single Sign On token credentials, and linking it to
//! an `atlasserver` user id.
//! # Providers supported
//!  - Facebook
//!  - Sign In With Apple
//!
//! # Example
//! This module just needs to be included in the `CustomServer` implementation for your server
//! struct, and the `SsoResource` needs to be included in the `Resources`.
//!
//! As is the case with all `atlasserver` modules, `ModuleResources` must also be implemented.
//!
//! `AtlasSso` also depends on `UserLoginResource`, so it should be included in the `CustomServer`
//! implementation.
//!
//! ```rust
//! use atlasserver::{
//!     CustomServer, CustomModule, Module, ModuleResources,
//!     hlist, Hlist,
//!     userlogin::{
//!         user::in_memory::InMemoryUserDB, sessions::InMemorySessionDB,
//!         UserLogin, UserLoginResource,
//!     }
//! };
//! use atlas_sso::{AtlasSso, SsoResource, InMemorySsoDB};
//! use std::sync::Arc;
//! # use futures::future::{Abortable, AbortHandle};
//!
//! struct MyServer{
//!     resources: <Self as CustomServer>::Resources,
//! }
//!
//! impl CustomServer for MyServer {
//!     type Resources = Hlist![Arc<SsoResource>, Arc<UserLoginResource>];
//!
//!     const MODULES: &'static [Module<Self>] = &[
//!          Module {
//!              name: "sso",
//!              call: AtlasSso::create_filter,
//!          },
//!          Module {
//!              name: "userlogin",
//!              call: UserLogin::create_filter,
//!          }
//!     ];
//!
//!     fn get_resources(&self) -> &Self::Resources {
//!         &self.resources
//!     }
//! }
//!
//! impl ModuleResources<AtlasSso> for MyServer {
//!     fn get_server_resources(&self) -> <AtlasSso as CustomModule>::Resources {
//!         let (reshaped, _) = self.get_resources().clone().sculpt();
//!         reshaped
//!     }
//! }
//!
//! impl ModuleResources<UserLogin> for MyServer {
//!     fn get_server_resources(&self) -> <UserLogin as CustomModule>::Resources {
//!         let (reshaped, _) = self.get_resources().clone().sculpt();
//!         reshaped
//!     }
//! }
//!
//! #[tokio::main]
//! async fn main() {
//!     let user_db = Arc::new(InMemoryUserDB::default());
//!     let session_db = Arc::new(InMemorySessionDB::default());
//!     let sso_db = Arc::new(InMemorySsoDB::default());
//!     
//!     let server = MyServer {
//!         resources: hlist![
//!             Arc::new(SsoResource::new(sso_db, user_db.clone())),
//!             Arc::new(UserLoginResource::new(session_db, user_db)),
//!         ]
//!     };
//!
//!     let future = atlasserver::init(Arc::new(server), ([0, 0, 0, 0], 8080));
//! # let (abort_handle, abort_registration) = AbortHandle::new_pair();
//! # let future = Abortable::new(future, abort_registration);
//! # abort_handle.abort();
//!     future.await;
//! }
//! ```

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
pub mod fb;
pub mod schema;
pub mod siwa;

pub use self::db::{
	DynamoSsoDB, InMemorySsoDB, Provider, SetSsoResult, SsoDB,
	SsoEntry, SsoKey,
};
use atlasserver::{
	userlogin::{
		user::{User, UserDB},
		UserLoginResource,
	},
	CustomModule, Hlist, ModuleResources,
};
use error::{Error, Result};
use std::{collections::HashMap, sync::Arc};
use tracing::instrument;
use warp::{filters::BoxedFilter, Reply};

pub struct AtlasSso;

impl CustomModule for AtlasSso {
	type Resources =
		Hlist![Arc<SsoResource>, Arc<UserLoginResource>,];

	fn create_filter<S: ModuleResources<Self>>(
		server: Arc<S>,
	) -> BoxedFilter<(Box<dyn Reply>,)> {
		let (reshaped, _): (Self::Resources, _) =
			server.get_server_resources().sculpt();
		let (sso_resource, user_resource) = reshaped.into_tuple2();

		siwa::create_filters_siwa(&user_resource, sso_resource)
	}
}

pub struct SsoResource {
	sso_db: Arc<dyn SsoDB>,
	users: Arc<dyn UserDB>,
	pub fb_callbacks: Arc<dyn fb::FbCallbacks>,
}

impl SsoResource {
	#[must_use]
	pub fn new(
		sso_db: Arc<dyn SsoDB>,
		users: Arc<dyn UserDB>,
		fb_callbacks: Arc<dyn fb::FbCallbacks>,
	) -> Self {
		Self {
			sso_db,
			users,
			fb_callbacks,
		}
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
