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

use atlasserver::{
	userlogin::UserLoginResource, CustomModule, ModuleResources,
};
use frunk::Hlist;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::instrument;
use warp::{filters::BoxedFilter, Filter, Rejection, Reply};

// see https://developer.apple.com/documentation/appstoreservernotifications/unified_receipt
#[derive(Deserialize, Serialize, Debug)]
struct UnifiedReceipt {
	pub environment: String,
	pub status: Option<u32>,
	pub latest_receipt: Option<String>,
	// pub latest_receipt_info
	// pub pending_renewal_info
}

// see https://developer.apple.com/documentation/appstoreservernotifications/responsebody
#[derive(Deserialize, Serialize, Debug)]
struct AppleServerNotification {
	pub notification_type: String,
	pub environment: String,
	pub bid: String,
	pub bvrs: String,
	pub password: Option<String>,
	pub auto_renew_adam_id: Option<String>,
	pub auto_renew_product_id: Option<String>,
	pub auto_renew_status: Option<String>,
	pub auto_renew_status_change_date: Option<String>,
	pub auto_renew_status_change_date_ms: Option<String>,
	pub auto_renew_status_change_date_pst: Option<String>,
	pub expiration_intent: Option<u32>,
	pub unified_receipt: Option<UnifiedReceipt>,
}

pub struct AppleServerNotificationModule {}

impl CustomModule for AppleServerNotificationModule {
	type Resources = Hlist!(Arc<UserLoginResource>);

	fn create_filter<S: ModuleResources<Self>>(
		server: std::sync::Arc<S>,
	) -> BoxedFilter<(Box<dyn Reply>,)> {
		let (user_resource, _): (Arc<UserLoginResource>, _) =
			server.get_server_resources().pluck();

		let store =
			warp::path!("atlas" / "apple-server-notifications")
				.and(warp::post())
				.and(warp::body::json::<AppleServerNotification>())
				.and(warp::any().map(move || user_resource.clone()))
				.and_then(callback_filter_fn);

		store
			.map(|reply| -> Box<dyn Reply> { Box::new(reply) })
			.boxed()
	}
}

//TODO: https://developer.apple.com/documentation/appstoreservernotifications/responsebody
#[instrument(skip(_resource))]
async fn callback_filter_fn(
	msg: AppleServerNotification,
	_resource: Arc<UserLoginResource>,
) -> Result<impl Reply, Rejection> {
	tracing::info!("apple-server-notifications");

	Ok(warp::reply::with_status("", warp::hyper::StatusCode::OK)
		.into_response())
}
