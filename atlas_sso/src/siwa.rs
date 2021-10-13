use crate::{
	error::Result, schema, Provider, SetSsoResult, SsoEntry, SsoKey,
	SsoResource,
};
use atlasserver::{
	pbwarp,
	userlogin::{self, UserLoginResource},
};
use std::sync::Arc;
use tracing::instrument;
use warp::{reply, Filter, Rejection, Reply};

#[instrument(skip(sso), err)]
async fn siwa_id(
	user_id: String,
	request: schema::AppleSignInRequest,
	sso: Arc<SsoResource>,
) -> Result<schema::SsoIdResponse> {
	tracing::info!("siwa_id");

	let provider_id = request.userId.clone();

	let res = sign_in_with_apple::validate(
		request.userId,
		request.token,
		false,
	)
	.await;

	let res = if sign_in_with_apple::is_expired(&res) {
		return Ok(schema::SsoIdResponse {
			result: schema::SsoIdResponse_Result::OUTDATED_TOKEN,
			..schema::SsoIdResponse::default()
		});
	} else {
		res?
	};

	tracing::info!("siwa_id res: {:?}", res);

	let res = sso
		.set_sso(SsoEntry {
			user_id: user_id.clone(),
			provider_id,
			provider: Provider::SignInWithApple,
		})
		.await?;

	Ok(schema::SsoIdResponse {
		result: match res {
			SetSsoResult::Success => schema::SsoIdResponse_Result::OK,
			SetSsoResult::AlreadyAssignedDifferently => {
				schema::SsoIdResponse_Result::ALREADY_ASSIGNED
			}
		},
		..schema::SsoIdResponse::default()
	})
}

#[instrument(skip(sso), err)]
async fn siwa_disconnect(
	user_id: String,
	request: schema::AppleSignInRequest,
	sso: Arc<SsoResource>,
) -> Result<schema::DisconnectSiwaResponse> {
	tracing::info!("siwa_disconnect");

	let provider_id = request.userId.clone();

	let res = sign_in_with_apple::validate(
		request.userId,
		request.token,
		false,
	)
	.await?;

	tracing::debug!("siwa validate res: {:?}", res);

	let user = sso
		.get_user(SsoKey {
			provider_id: provider_id.clone(),
			provider: Provider::SignInWithApple,
		})
		.await?;

	if user.id != user_id {
		return Err(crate::Error::Generic(String::from(
			"invalid user",
		)));
	}

	sso.remove_entry(SsoEntry {
		user_id: user.id,
		provider: Provider::SignInWithApple,
		provider_id,
	})
	.await?;

	Ok(schema::DisconnectSiwaResponse {
		success: true,
		..schema::DisconnectSiwaResponse::default()
	})
}

#[instrument(skip(sso), err)]
async fn siwa_login(
	request: schema::AppleSignInRequest,
	sso: Arc<SsoResource>,
) -> Result<atlasserver::schema::RegisterResponse> {
	tracing::info!("siwa_login: {:?}", request);

	let provider_id = request.userId.clone();

	let res = sign_in_with_apple::validate(
		request.userId,
		request.token,
		false,
	)
	.await?;

	tracing::info!("siwa_login res: {:?}", res);

	let user = sso
		.get_user(SsoKey {
			provider_id,
			provider: Provider::SignInWithApple,
		})
		.await?;

	Ok(atlasserver::schema::RegisterResponse {
		user: Some(atlasserver::schema::UserCredentials {
			id: user.id,
			secret: user.secret,
			..atlasserver::schema::UserCredentials::default()
		})
		.into(),
		..atlasserver::schema::RegisterResponse::default()
	})
}

async fn siwa_id_filter_fn(
	user_id: String,
	request: schema::AppleSignInRequest,
	sso: Arc<SsoResource>,
) -> std::result::Result<impl Reply, Rejection> {
	tracing::info!("siwa_id_filter_fn: {:?}", request);

	match siwa_id(user_id, request, sso).await {
		Ok(response) => {
			Ok(pbwarp::protobuf_reply(&response, None)
				.into_response())
		}
		Err(e) => {
			tracing::error!("siwa auth error: {}", e);
			Ok(warp::reply::with_status(
				reply(),
				warp::hyper::StatusCode::INTERNAL_SERVER_ERROR,
			)
			.into_response())
		}
	}
}

async fn siwa_disconnect_filter_fn(
	user_id: String,
	request: schema::AppleSignInRequest,
	sso: Arc<SsoResource>,
) -> std::result::Result<impl Reply, Rejection> {
	tracing::info!("siwa_disconnect_filter_fn: {:?}", request);

	match siwa_disconnect(user_id, request, sso).await {
		Ok(response) => {
			Ok(pbwarp::protobuf_reply(&response, None)
				.into_response())
		}
		Err(e) => {
			tracing::error!("siwa disconnect error: {}", e);
			Ok(warp::reply::with_status(
				reply(),
				warp::hyper::StatusCode::INTERNAL_SERVER_ERROR,
			)
			.into_response())
		}
	}
}

async fn siwa_login_filter_fn(
	user_id: String,
	request: schema::AppleSignInRequest,
	sso: Arc<SsoResource>,
) -> std::result::Result<impl Reply, Rejection> {
	//TODO: lockdown account and mark with superceding old account
	tracing::info!(
		"siwa_login_filter_fn: {:?} [{}]",
		request,
		user_id
	);

	match siwa_login(request, sso).await {
		Ok(response) => {
			Ok(pbwarp::protobuf_reply(&response, None)
				.into_response())
		}
		Err(e) => {
			tracing::error!("siwa login error: {}", e);
			Ok(warp::reply::with_status(
				reply(),
				warp::hyper::StatusCode::INTERNAL_SERVER_ERROR,
			)
			.into_response())
		}
	}
}

#[derive(Debug, serde::Deserialize)]
struct AppleSIWAServer2ServerPayload {
	pub payload: String,
}

// see https://developer.apple.com/documentation/sign_in_with_apple/processing_changes_for_sign_in_with_apple_accounts
async fn server_to_server_filter_fn(
	data: AppleSIWAServer2ServerPayload,
) -> std::result::Result<impl Reply, Rejection> {
	tracing::info!("server2server: {:?}", data);

	let res = sign_in_with_apple::decode_token::<
		sign_in_with_apple::ClaimsServer2Server,
	>(data.payload, true)
	.await;

	match res {
		Ok(res) => tracing::info!("server2server: {:?}", res),
		Err(e) => tracing::error!("server2server error: {}", e),
	};

	Ok(warp::reply::with_status("", warp::hyper::StatusCode::OK)
		.into_response())
}

pub fn create_filters_siwa(
	user_resource: &Arc<UserLoginResource>,
	sso_res: Arc<SsoResource>,
) -> warp::filters::BoxedFilter<(Box<dyn warp::Reply>,)> {
	let sso = warp::any().map(move || sso_res.clone());

	let server_to_server_filter =
		warp::path!("atlas" / "siwa" / "server2server")
			.and(warp::post())
			.and(warp::body::json())
			.and_then(server_to_server_filter_fn);

	let siwa_id_filter = warp::path!("siwa" / "id")
		.and(userlogin::session_filter(user_resource.clone()))
		.and(warp::post())
		.and(pbwarp::protobuf_body::<schema::AppleSignInRequest>())
		.and(sso.clone())
		.and_then(siwa_id_filter_fn);
	let siwa_disconnect_filter = warp::path!("siwa" / "disconnect")
		.and(userlogin::session_filter(user_resource.clone()))
		.and(warp::post())
		.and(pbwarp::protobuf_body::<schema::AppleSignInRequest>())
		.and(sso.clone())
		.and_then(siwa_disconnect_filter_fn);
	let siwa_login_filter = warp::path!("siwa" / "login")
		.and(userlogin::session_filter(user_resource.clone()))
		.and(warp::post())
		.and(pbwarp::protobuf_body::<schema::AppleSignInRequest>())
		.and(sso)
		.and_then(siwa_login_filter_fn);

	siwa_id_filter
		.or(siwa_disconnect_filter)
		.or(siwa_login_filter)
		.or(server_to_server_filter)
		.map(|reply| -> Box<dyn Reply> { Box::new(reply) })
		.boxed()
}
