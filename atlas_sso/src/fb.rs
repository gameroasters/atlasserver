//TODO: move into `atlas_sso` and communicate via event callbacks to s4backend

// use crate::s4module::player_state::PlayerStateResource; //TODO: Facebook connect call
use crate::error::{Error, Result};
use crate::{
	schema::{self, SsoIdResponse, SsoIdResponse_Result},
	Provider, SetSsoResult, SsoEntry, SsoKey, SsoResource,
};
use async_trait::async_trait;
use atlasserver::userlogin::UserId;
use fb_api::types::{Picture, User};
use fb_api::{FBGraphAPI, GraphAPI};
use std::sync::Arc;
use tracing::instrument;

#[async_trait]
pub trait FbCallbacks {
	async fn on_fb_connected(
		&self,
		user_id: UserId,
		fb_me: User,
	) -> Result<()>;
	async fn on_avatar_fetched(
		&self,
		user_id: UserId,
		picture: Picture,
	) -> Result<()>;
}

#[instrument(skip(sso, request, callbacks), err)]
pub async fn facebook_id<'a>(
	user_id: String,
	request: schema::FacebookIdRequest,
	sso: Arc<SsoResource>,
	callbacks: impl FbCallbacks,
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

	callbacks.on_fb_connected(user_id.clone(), me).await?;

	if let Some(pic) = api
		.my_picture(&request.token)
		.await
		.map_err(|e| Error::Fb(e.to_string()))?
		.data
	{
		callbacks.on_avatar_fetched(user_id, pic).await?;
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
