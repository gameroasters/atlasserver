use crate::{
	error::Result,
	fcmtoken::{FcmToken, FcmTokenDB},
	schema,
};
use fcm::{Client, MessageBuilder, NotificationBuilder};
use schema::FcmTokenStoreResponse;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::instrument;

pub struct PushNotificationResource {
	tokens: Arc<dyn FcmTokenDB>,
	api_key: String,
}

#[derive(Default, Debug)]
pub struct PushNotificationContext {
	pub user_id: Option<String>,
	pub district_id: Option<u8>,
}

impl PushNotificationContext {
	#[must_use]
	pub fn with_user_id(user_id: String) -> Self {
		Self {
			user_id: Some(user_id),
			..Self::default()
		}
	}
}

impl PushNotificationResource {
	#[must_use]
	pub fn new(tokens: Arc<dyn FcmTokenDB>, api_key: String) -> Self {
		Self { tokens, api_key }
	}

	/// # Errors
	/// will return an error if the tokens database failed to set the token
	#[instrument(skip(self))]
	pub async fn set(
		&self,
		user_id: &str,
		token: &str,
	) -> Result<schema::FcmTokenStoreResponse> {
		let token = FcmToken {
			id: user_id.to_string(),
			token: token.to_string(),
		};

		self.tokens.set(token).await?;

		tracing::info!("token-stored");

		Ok(FcmTokenStoreResponse {
			success: true,
			..FcmTokenStoreResponse::default()
		})
	}

	pub async fn get_token(&self, user_id: &str) -> Option<FcmToken> {
		self.tokens.get(user_id).await
	}

	#[instrument(skip(self))]
	pub async fn send_message(
		&self,
		token: &str,
		title: String,
		body: String,
		msg_type: String,
		context_data: Option<PushNotificationContext>,
	) -> Result<()> {
		let client = Client::new();

		//Note: previously we had to pass the payload as data and notification,
		// not sure anymore why, maybe its needed for android which we did not look into yet
		let mut map = HashMap::with_capacity(3);
		map.insert("Title", title.clone());
		map.insert("Body", body.clone());

		//TODO: this is project specific, abstract away
		if let Some(context_data) = context_data {
			map.insert("msg-type", msg_type);
			if let Some(user_id) = context_data.user_id {
				map.insert("user-id", user_id);
			}

			if let Some(district_id) = context_data.district_id {
				map.insert("district-id", district_id.to_string());
			}
		}

		let mut notification = NotificationBuilder::new();

		//note: on ios this is used and shown
		notification.title(title.as_str());
		notification.body(body.as_str());
		notification.sound("default");

		let mut builder = MessageBuilder::new(&self.api_key, token);
		builder.data(&map)?;
		builder.notification(notification.finalize());
		let response = client.send(builder.finalize()).await?;

		tracing::info!("fcm: {:?}", response);

		Ok(())
	}
}
