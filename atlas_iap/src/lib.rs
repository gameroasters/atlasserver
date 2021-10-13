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
