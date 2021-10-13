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
use std::{collections::HashMap, sync::Arc};
use tracing::instrument;
use warp::{filters::BoxedFilter, Filter, Rejection, Reply};

pub struct IronsourceCallbackModule {}

impl CustomModule for IronsourceCallbackModule {
	type Resources = Hlist!(Arc<UserLoginResource>);

	fn create_filter<S: ModuleResources<Self>>(
		server: std::sync::Arc<S>,
	) -> BoxedFilter<(Box<dyn Reply>,)> {
		let (user_resource, _): (Arc<UserLoginResource>, _) =
			server.get_server_resources().pluck();

		let store = warp::path!("atlas" / "ads" / "callback")
			.and(warp::get())
			.and(warp::query::<HashMap<String, String>>())
			.and(warp::any().map(move || user_resource.clone()))
			.and_then(callback_filter_fn);

		store
			.map(|reply| -> Box<dyn Reply> { Box::new(reply) })
			.boxed()
	}
}

//TODO: https://developers.is.com/ironsource-mobile/ios/server-to-server-callback-setting/#step-3
#[instrument(skip(_resource))]
async fn callback_filter_fn(
	params: HashMap<String, String>,
	_resource: Arc<UserLoginResource>,
) -> Result<impl Reply, Rejection> {
	tracing::info!("ads-callback");

	//TODO: check signature using private key

	//TODO: check IP
	//see https://developers.is.com/ironsource-mobile-general/handling-server-to-server-callback-events/#step-5

	Ok(warp::reply::with_status(
		format!(
			"{}:OK",
			params.get("event").cloned().unwrap_or_default()
		),
		warp::hyper::StatusCode::OK,
	)
	.into_response())

	// Ok(warp::reply::with_status(
	// 	String::from("failed to set token"),
	// 	warp::hyper::StatusCode::BAD_REQUEST,
	// )
	// .into_response())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {

	// #[tokio::test]
	// async fn test_filter() {
	// 	let mut hashmap: HashMap<String, Session> = HashMap::new();

	// 	hashmap.insert("sid".to_string(), Session::new("uid"));

	// 	let sessions = Arc::new(InMemorySessionDB {
	// 		db: Arc::new(Mutex::new(hashmap)),
	// 	});

	// 	let server = Arc::new(Server {
	// 		resources: hlist![
	// 			Arc::new(UserLoginResource::new(
	// 				sessions,
	// 				Arc::new(InMemoryUserDB::default()),
	// 			)),
	// 			Arc::new(PushNotificationResource::new(
	// 				Arc::new(InMemoryFcmTokenDB::default(),),
	// 				String::default()
	// 			))
	// 		],
	// 	});

	// 	let module = Server::get_module("fcm").unwrap();
	// 	let filter = (module.call)(server.clone());

	// 	let route = filter.recover(rejection::handle_rejection);

	// 	let request = FcmTokenStoreRequest {
	// 		token: "hello".to_string(),
	// 		..FcmTokenStoreRequest::default()
	// 	};

	// 	let reply = warp::test::request()
	// 		.method("POST")
	// 		.body(request.write_to_bytes().unwrap())
	// 		.header(atlasserver::userlogin::HEADER_SESSION, "sid")
	// 		.path("/fcm/store")
	// 		.reply(&route)
	// 		.await;

	// 	assert_eq!(reply.status(), 200);
	// 	assert!(
	// 		FcmTokenStoreResponse::parse_from_bytes(reply.body())
	// 			.unwrap()
	// 			.success
	// 	);
	// }
}
