use crate::error::{Error, Result};
use crate::{
	schema::{self, SsoIdResponse, SsoIdResponse_Result},
	Provider, SetSsoResult, SsoEntry, SsoKey, SsoResource,
};
use async_trait::async_trait;
use atlasserver::userlogin::{UserId, UserLoginResource};
use atlasserver::{pbwarp, userlogin};
use fb_api::types::{Picture, User};
use fb_api::{FBGraphAPI, GraphAPI};
use std::sync::Arc;
use tracing::instrument;
use warp::{reply, Filter, Rejection, Reply};

///Used to provide callbacks called inside the various fb functions
#[async_trait]
pub trait FbCallbacks: Send + Sync {
	///Called when account is successfully connected to facebook, not on every login
	///Only applies if the `SsoResponse` is not `AlreadyAssignedDifferently`, meaning that another account is already tied to the given facebook id
	async fn on_fb_connected(
		&self,
		user_id: UserId,
		fb_me: User,
	) -> Result<()>;
	///Called once facebook avatar url is fetched successfully
	async fn on_avatar_fetched(
		&self,
		user_id: UserId,
		picture: Picture,
	) -> Result<()>;
}

#[instrument(skip(sso, request), err)]
pub async fn facebook_id(
	user_id: String,
	request: schema::FacebookIdRequest,
	sso: Arc<SsoResource>,
) -> Result<SsoIdResponse> {
	tracing::info!("facebook_id: {:?}", request);

	let api = FBGraphAPI::default();

	let me = api
		.me(&request.token)
		.await
		.map_err(|e| Error::Fb(e.to_string()))?;

	tracing::info!(
		target: "fb-auth",
		me = ?me.id,
		name = ?me.name
	);

	if let Some(fb_id) = me.id.as_ref() {
		let res = sso
			.set_sso(SsoEntry {
				user_id: user_id.clone(),
				provider_id: fb_id.clone(),
				provider: Provider::Facebook,
			})
			.await?;

		tracing::info!(
			target: "sso-fb",
			res = ?res,
		);

		if matches!(res, SetSsoResult::AlreadyAssignedDifferently) {
			return Ok(SsoIdResponse {
				result: SsoIdResponse_Result::ALREADY_ASSIGNED,
				..SsoIdResponse::default()
			});
		}
	} else {
		return Err(Error::Fb("invalid fb id".to_string()));
	}

	sso.fb_callbacks
		.on_fb_connected(user_id.clone(), me)
		.await?;

	if let Some(pic) = api
		.my_picture(&request.token)
		.await
		.map_err(|e| Error::Fb(e.to_string()))?
		.data
	{
		sso.fb_callbacks.on_avatar_fetched(user_id, pic).await?;
	}

	Ok(SsoIdResponse {
		result: SsoIdResponse_Result::OK,
		..SsoIdResponse::default()
	})
}

#[instrument(skip(sso), err)]
pub async fn facebook_login(
	request: schema::FacebookIdRequest,
	sso: Arc<SsoResource>,
) -> Result<atlasserver::schema::RegisterResponse> {
	tracing::info!("facebook_login: {:?}", request);

	let api = FBGraphAPI::default();

	let me = api
		.me(&request.token)
		.await
		.map_err(|e| Error::Fb(e.to_string()))?;

	tracing::info!(
		target: "fb-auth",
		me = ?me.id,
		name = ?me.name
	);

	if let Some(fb_id) = me.id.as_ref() {
		let user = sso.get_user(SsoKey::facebook(fb_id)).await?;

		Ok(atlasserver::schema::RegisterResponse {
			user: Some(atlasserver::schema::UserCredentials {
				id: user.id,
				secret: user.secret,
				..atlasserver::schema::UserCredentials::default()
			})
			.into(),
			..atlasserver::schema::RegisterResponse::default()
		})
	} else {
		Err(Error::Fb("invalid fb id".to_string()))
	}
}

async fn facebook_id_filter_fn(
	user_id: String,
	request: schema::FacebookIdRequest,
	sso: Arc<SsoResource>,
) -> std::result::Result<impl Reply, Rejection> {
	tracing::info!("facebook_id_filter_fn: {:?}", request);

	match facebook_id(user_id, request, sso).await {
		Ok(response) => {
			Ok(pbwarp::protobuf_reply(&response, None)
				.into_response())
		}
		Err(e) => {
			tracing::error!("fb auth error: {}", e);
			Ok(warp::reply::with_status(
				reply(),
				warp::hyper::StatusCode::INTERNAL_SERVER_ERROR,
			)
			.into_response())
		}
	}
}

async fn facebook_login_filter_fn(
	user_id: String,
	request: schema::FacebookIdRequest,
	sso: Arc<SsoResource>,
) -> std::result::Result<impl Reply, Rejection> {
	//TODO: lockdown account and mark with superceding old account
	tracing::info!(
		"facebook_login_filter_fn: {:?} [{}]",
		request,
		user_id
	);

	match facebook_login(request, sso).await {
		Ok(response) => {
			Ok(pbwarp::protobuf_reply(&response, None)
				.into_response())
		}
		Err(e) => {
			tracing::error!("fb login error: {}", e);
			Ok(warp::reply::with_status(
				reply(),
				warp::hyper::StatusCode::INTERNAL_SERVER_ERROR,
			)
			.into_response())
		}
	}
}

pub fn create_filters_fb(
	user_resource: &Arc<UserLoginResource>,
	sso_res: Arc<SsoResource>,
) -> warp::filters::BoxedFilter<(Box<dyn warp::Reply>,)> {
	let sso = warp::any().map(move || sso_res.clone());

	let facebook_id_filter = warp::path!("facebook" / "id")
		.and(userlogin::session_filter(user_resource.clone()))
		.and(warp::post())
		.and(pbwarp::protobuf_body::<schema::FacebookIdRequest>())
		.and(sso.clone())
		.and_then(facebook_id_filter_fn);

	let facebook_login_filter = warp::path!("facebook" / "login")
		.and(userlogin::session_filter(user_resource.clone()))
		.and(warp::post())
		.and(pbwarp::protobuf_body::<schema::FacebookIdRequest>())
		.and(sso)
		.and_then(facebook_login_filter_fn);

	facebook_id_filter
		.or(facebook_login_filter)
		.map(|reply| -> Box<dyn Reply> { Box::new(reply) })
		.boxed()
}
