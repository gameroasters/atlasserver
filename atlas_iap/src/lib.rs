//! An atlas module used for validating purchases made through Unity's In App Purchases plugin.
//! Validated purchases get stored in a receipts database, and trigger a callback which can be
//! registered to handle giving the players resource, or marking them as subscribed.
//!
//! # Stores supported
//!  - Google Play
//!  - Apple App Store
//!
//! # Example
//! This module just needs to be included in the `CustomServer` implementation for your server
//! struct, and the `IapResource` needs to be included in the `Resources`.
//!
//! As is the case with all `atlasserver` modules, `ModuleResources` must also be implemented.
//!
//! `Iap` also depends on `UserLoginResource`, so it should be included in the `CustomServer`
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
//! use atlas_iap::{
//!     Iap, IapResource, IapEventHandler, InMemoryReceiptDB, Receipt
//! };
//! use async_trait::async_trait;
//! use std::sync::Arc;
//! # use futures::future::{Abortable, AbortHandle};
//!
//! struct MyServer{
//!     resources: <Self as CustomServer>::Resources,
//! }
//!
//! impl CustomServer for MyServer {
//!     type Resources = Hlist![Arc<IapResource>, Arc<UserLoginResource>];
//!
//!     const MODULES: &'static [Module<Self>] = &[
//!          Module {
//!              name: "iap",
//!              call: Iap::create_filter,
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
//! impl ModuleResources<Iap> for MyServer {
//!     fn get_server_resources(&self) -> <Iap as CustomModule>::Resources {
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
//! struct MyEventHandler;
//!
//! #[async_trait]
//! impl IapEventHandler for MyEventHandler {
//!     async fn on_valid_receipt(
//!         &self,
//!         receipt: &Receipt
//!     ) -> atlas_iap::error::Result<()> {
//!         // Handle valid receipt purchases here
//! # Ok(())
//!     }
//! }
//!
//! #[tokio::main]
//! async fn main() {
//!     let user_db = Arc::new(InMemoryUserDB::default());
//!     let session_db = Arc::new(InMemorySessionDB::default());
//!     let receipt_db = Arc::new(InMemoryReceiptDB::default());
//!
//!     let mut iap_resource =
//!         IapResource::new(
//!             receipt_db,
//!             Some(String::from("apple_secret")),
//!             None
//!         )
//!         .unwrap();
//!
//!     iap_resource.set_event_handler(Arc::new(MyEventHandler));
//!     
//!     let server = MyServer {
//!         resources: hlist![
//!             Arc::new(iap_resource),
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

pub mod error;
mod receipt;
mod resource;
pub mod schema;
mod utc_time;

use async_trait::async_trait;
use atlasserver::{
	userlogin::UserLoginResource, CustomModule, Hlist,
};
use chrono::Utc;
use iap::UnityPurchaseReceipt;
use std::sync::Arc;
use warp::{filters::BoxedFilter, Filter, Rejection, Reply};

pub use receipt::{
	dynamodb::DynamoReceiptDB, in_memory::InMemoryReceiptDB,
	Platform, Receipt,
};
pub use resource::IapResource;

pub struct Iap;

#[async_trait]
pub trait IapEventHandler: Send + Sync {
	async fn on_valid_receipt(
		&self,
		_: &Receipt,
	) -> Result<(), error::Error>;
}

impl CustomModule for Iap {
	type Resources = Hlist!(Arc<IapResource>, Arc<UserLoginResource>);

	fn create_filter<S: atlasserver::ModuleResources<Self>>(
		server: std::sync::Arc<S>,
	) -> BoxedFilter<(Box<dyn Reply>,)> {
		let (reshaped, _): (Self::Resources, _) =
			server.get_server_resources().sculpt();
		let (iap_res, user_res) = reshaped.into_tuple2();

		let iap_res = warp::any().map(move || iap_res.clone());

		let validation_filter =
			warp::path!("atlas" / "iap" / "purchase")
				.and(atlasserver::userlogin::session_filter(user_res))
				.and(warp::body::json::<UnityPurchaseReceipt>())
				.and(iap_res)
				.and_then(validation_filter_fn);

		validation_filter
			.map(|reply| -> Box<dyn Reply> { Box::new(reply) })
			.boxed()
	}
}

async fn validation_filter_fn(
	user_id: String,
	receipt: UnityPurchaseReceipt,
	resource: Arc<IapResource>,
) -> Result<impl Reply, Rejection> {
	match resource
		.validate_purchase(&user_id, &receipt, Utc::now())
		.await
	{
		Ok(response) => {
			return Ok(atlasserver::pbwarp::protobuf_reply(
				&response, None,
			)
			.into_response());
		}
		Err(err) => {
			tracing::error!("purchase validation error: {}", err);
		}
	};
	Ok(warp::reply::with_status(
		String::from("failed to handle subscription validation"),
		warp::hyper::StatusCode::BAD_REQUEST,
	)
	.into_response())
}
