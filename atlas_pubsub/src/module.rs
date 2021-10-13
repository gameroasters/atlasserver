use super::{
	resources::{PubSubPublishResource, PubSubSubscriberResource},
	PubSubPublish,
};
use atlasserver::{
	userlogin::{UserId, UserLoginResource},
	CustomModule,
};
use frunk::Hlist;
use std::sync::Arc;
use warp::{reply, ws::WebSocket, Filter, Rejection, Reply};

/// module that adds the following endpoints:
/// * atlas/pubsub/subscribe/{session}
/// * atlas/pubsub/publish/{topic}/msg/{msg} (only with `publish-endpoint` feature)
pub struct PubSubModule {}

impl CustomModule for PubSubModule {
	type Resources = Hlist![
		Arc<UserLoginResource>,
		Arc<PubSubSubscriberResource>,
		Arc<PubSubPublishResource>
	];

	fn create_filter<S: atlasserver::ModuleResources<Self>>(
		server: std::sync::Arc<S>,
	) -> warp::filters::BoxedFilter<(Box<dyn warp::Reply>,)> {
		let (reshaped, _): (Self::Resources, _) =
			server.get_server_resources().sculpt();
		let (userlogin, (subscriber, publisher)) =
			reshaped.into_tuple2();

		let subscribe = warp::any().map(move || subscriber.clone());
		let publisher = warp::any().map(move || publisher.clone());

		let filter_ws_connect =
			warp::path!("atlas" / "pubsub" / "subscribe" / String)
				.and(warp::ws())
				.and(subscribe)
				.and(warp::any().map(move || userlogin.clone()))
				.and_then(pubsub_filter);

		//TODO: remove once done with debugging
		// use `publish-endpoint`-cfg
		let filter_ws_publish = warp::path!(
			"atlas" / "pubsub" / "publish" / String / "msg" / String
		)
		.and(publisher)
		.and_then(pubsub_publish_filter);

		filter_ws_connect
			.or(filter_ws_publish)
			.map(|reply| -> Box<dyn Reply> { Box::new(reply) })
			.boxed()
	}
}

async fn pubsub_filter(
	session: String,
	ws: warp::ws::Ws,
	pubsub: Arc<PubSubSubscriberResource>,
	userlogin: Arc<UserLoginResource>,
) -> Result<impl Reply, Rejection> {
	tracing::info!("pubsub_filter: {}", session);

	match userlogin.validate_session(&session).await {
		atlasserver::userlogin::SessionValidationResult::Ok {
			user_id,
		} => Ok(ws
			.on_upgrade(move |socket| {
				user_connected(pubsub, socket, user_id)
			})
			.into_response()),
		atlasserver::userlogin::SessionValidationResult::Invalid
		| atlasserver::userlogin::SessionValidationResult::Unknown => {
			Ok(warp::reply::with_status(
				reply(),
				warp::hyper::StatusCode::INTERNAL_SERVER_ERROR,
			)
			.into_response())
		}
	}
}

async fn pubsub_publish_filter(
	topic: String,
	msg: String,
	pubsub: Arc<PubSubPublishResource>,
) -> Result<impl Reply, Rejection> {
	pubsub.publish_text(&topic, &msg).await;
	Ok(reply())
}

async fn user_connected(
	pubsub: Arc<PubSubSubscriberResource>,
	ws: WebSocket,
	user_id: UserId,
) {
	pubsub.on_connect(&user_id, ws).await;
}
