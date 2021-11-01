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

	#[instrument(skip(self, payload_data))]
	pub async fn send_message(
		&self,
		token: &str,
		title: String,
		body: String,
		payload_data: Option<&HashMap<&str, String>>,
	) -> Result<()> {
		let client = Client::new();
		let mut notification = NotificationBuilder::new();

		//NOTE: on ios this is used and shown
		notification.title(title.as_str());
		notification.body(body.as_str());
		notification.sound("default");

		let mut builder = MessageBuilder::new(&self.api_key, token);
		if let Some(payload_data) = payload_data {
			builder.data(&payload_data)?;
		}
		builder.notification(notification.finalize());
		let response = client.send(builder.finalize()).await?;

		tracing::info!("fcm: {:?}", response);

		Ok(())
	}
}
