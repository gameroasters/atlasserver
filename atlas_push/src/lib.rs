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

pub mod dynamo;
pub mod error;
mod fcmtoken;
pub mod in_memory;
pub mod resource;
pub mod schema;

use atlasserver::{
	pbwarp,
	userlogin::{self, UserLoginResource},
	CustomModule, ModuleResources,
};
use frunk::Hlist;
use resource::PushNotificationResource;
use schema::FcmTokenStoreRequest;
use std::sync::Arc;
use warp::{filters::BoxedFilter, Filter, Rejection, Reply};

pub struct FcmTokens {}

impl CustomModule for FcmTokens {
	type Resources =
		Hlist!(Arc<UserLoginResource>, Arc<PushNotificationResource>);

	fn create_filter<S: ModuleResources<Self>>(
		server: std::sync::Arc<S>,
	) -> BoxedFilter<(Box<dyn Reply>,)> {
		let (user_resource, remaining): (Arc<UserLoginResource>, _) =
			server.get_server_resources().pluck();

		let (fcm_resource, _) = remaining.pluck();

		let store = warp::path!("fcm" / "store")
			.and(warp::post())
			.and(userlogin::session_filter(user_resource))
			.and(pbwarp::protobuf_body::<FcmTokenStoreRequest>())
			.and(warp::any().map(move || fcm_resource.clone()))
			.and_then(fcm_store_filter_fn);

		store
			.map(|reply| -> Box<dyn Reply> { Box::new(reply) })
			.boxed()
	}
}

async fn fcm_store_filter_fn(
	user_id: String,
	request: FcmTokenStoreRequest,
	resource: Arc<PushNotificationResource>,
) -> Result<impl Reply, Rejection> {
	match resource.set(&user_id, &request.token).await {
		Ok(response) => {
			return Ok(pbwarp::protobuf_reply(&response, None)
				.into_response());
		}
		Err(err) => tracing::error!("{}", err),
	};
	Ok(warp::reply::with_status(
		String::from("failed to set token"),
		warp::hyper::StatusCode::BAD_REQUEST,
	)
	.into_response())
}

#[cfg(test)]
mod tests {
	#![allow(
        //TODO: https://github.com/rust-lang/rust-clippy/issues/7438
		clippy::semicolon_if_nothing_returned,
		clippy::unwrap_used
	)]
	use crate::{
		in_memory::InMemoryFcmTokenDB,
		resource::PushNotificationResource,
		schema::{
			FcmTokenStoreRequest, FcmTokenStoreResponse, Message,
		},
		FcmTokens,
	};
	use atlasserver::{
		rejection,
		userlogin::{
			sessions::{InMemorySessionDB, Session},
			user::in_memory::InMemoryUserDB,
			UserLogin, UserLoginResource,
		},
		CustomModule, CustomServer, Module, ModuleResources,
	};
	use frunk::{hlist, Hlist};
	use pretty_assertions::assert_eq;
	use std::{collections::HashMap, sync::Arc};
	use tokio::sync::Mutex;
	use warp::Filter;

	pub struct Server {
		#[allow(dead_code)]
		resources: <Server as CustomServer>::Resources,
	}

	impl CustomServer for Server {
		type Resources = Hlist!(
			Arc<UserLoginResource>,
			Arc<PushNotificationResource>
		);

		const MODULES: &'static [atlasserver::Module<Self>] = &[
			Module {
				call: FcmTokens::create_filter,
				name: "fcm",
			},
			Module {
				call: UserLogin::create_filter,
				name: "userlogin",
			},
		];

		fn get_resources(&self) -> &Self::Resources {
			&self.resources
		}
	}

	impl ModuleResources<UserLogin> for Server {
		fn get_server_resources(
			&self,
		) -> <UserLogin as CustomModule>::Resources {
			let (reshaped, _) = self.get_resources().clone().sculpt();
			reshaped
		}
	}

	impl ModuleResources<FcmTokens> for Server {
		fn get_server_resources(
			&self,
		) -> <FcmTokens as CustomModule>::Resources {
			let (reshaped, _) = self.get_resources().clone().sculpt();
			reshaped
		}
	}

	#[tokio::test]
	async fn test_filter() {
		let mut hashmap: HashMap<String, Session> = HashMap::new();

		hashmap.insert("sid".to_string(), Session::new("uid"));

		let sessions = Arc::new(InMemorySessionDB {
			db: Arc::new(Mutex::new(hashmap)),
		});

		let server = Arc::new(Server {
			resources: hlist![
				Arc::new(UserLoginResource::new(
					sessions,
					Arc::new(InMemoryUserDB::default()),
				)),
				Arc::new(PushNotificationResource::new(
					Arc::new(InMemoryFcmTokenDB::default(),),
					String::default()
				))
			],
		});

		let module = Server::get_module("fcm").unwrap();
		let filter = (module.call)(server.clone());

		let route = filter.recover(rejection::handle_rejection);

		let request = FcmTokenStoreRequest {
			token: "hello".to_string(),
			..FcmTokenStoreRequest::default()
		};

		let reply = warp::test::request()
			.method("POST")
			.body(request.write_to_bytes().unwrap())
			.header(atlasserver::userlogin::HEADER_SESSION, "sid")
			.path("/fcm/store")
			.reply(&route)
			.await;

		assert_eq!(reply.status(), 200);
		assert!(
			FcmTokenStoreResponse::parse_from_bytes(reply.body())
				.unwrap()
				.success
		);
	}

	#[tokio::test]
	async fn test_token_storage() {
		let tokens = Arc::new(InMemoryFcmTokenDB::default());

		let resource = PushNotificationResource::new(
			tokens.clone(),
			String::default(),
		);

		resource.set("uid", "howdy").await.unwrap();

		let db = tokens.db.lock().await;
		assert_eq!(
			db.get(&"uid".to_string()).unwrap().token,
			"howdy".to_string()
		);
	}
}
