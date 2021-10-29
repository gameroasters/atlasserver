use crate::{
	error::Result,
	fcmtoken::{FcmToken, FcmTokenDB},
	schema,
};
use fcm::{Client, MessageBuilder, NotificationBuilder};
use schema::FcmTokenStoreResponse;
use std::sync::Arc;
use std::{collections::HashMap, fmt::Debug};
use tracing::instrument;

pub struct PushNotificationResource {
	tokens: Arc<dyn FcmTokenDB>,
	api_key: String,
}

#[derive(Default, Debug)]
pub struct S4PushNotificationContext {
	pub user_id: Option<String>,
	pub district_id: Option<u8>,
}

impl S4PushNotificationContext {
	#[must_use]
	pub fn with_user_id(user_id: String) -> Self {
		Self {
			user_id: Some(user_id),
			..Self::default()
		}
	}
}

impl PushNotificationContext for S4PushNotificationContext {
	fn insert_into_payload(&self, map: &mut HashMap<&str, String>) {
		if let Some(user_id) = self.user_id.clone() {
			map.insert("user-id", user_id);
		}

		if let Some(district_id) = self.district_id {
			map.insert("district-id", district_id.to_string());
		}
	}
}

pub trait PushNotificationContext: Debug {
	///Use this to insert context into the map which is sent with the push notification payload
	///The payload map will be accessible on the receiver side
	fn insert_into_payload(
		&self,
		payload_map: &mut HashMap<&str, String>,
	);
}

///Provide same insert functionality as `PushNotificationContext`
///Can be used to provide a custom inserting implementation for structs which already implement `PushNotificationContext`
pub trait PushNotificationContextMapper<C>: Debug {
	///Use this to insert context into the map which is sent with the push notification payload
	///The payload map will be accessible on the receiver side
	fn insert_into_payload(
		&self,
		context: &C,
		payload_map: &mut HashMap<&str, String>,
	);
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

	///Same as send_message, but this function takes a context which implements the mapping itself
	#[instrument(skip(self))]
	pub async fn send_message_with_mapper<
		C: PushNotificationContext,
	>(
		&self,
		token: &str,
		title: String,
		body: String,
		msg_type: String,
		context_data: Option<C>,
	) -> Result<()> {
		let mut map = HashMap::with_capacity(5);
		if let Some(context_data) = context_data {
			context_data.insert_into_payload(&mut map);
		}

		self.send_message(token, title, body, msg_type, &mut map)
			.await
	}

	#[instrument(skip(self))]
	pub async fn send_message_with_custom_mapper<
		C: Debug,
		M: PushNotificationContextMapper<C>,
	>(
		&self,
		token: &str,
		title: String,
		body: String,
		msg_type: String,
		context_data: Option<C>, //TODO: Abstract away
		context_mapper: M,
	) -> Result<()> {
		let mut map = HashMap::with_capacity(5);
		if let Some(context_data) = context_data {
			context_mapper
				.insert_into_payload(&context_data, &mut map);
		}

		self.send_message(token, title, body, msg_type, &mut map)
			.await
	}

	#[instrument(skip(self, map))]
	async fn send_message(
		&self,
		token: &str,
		title: String,
		body: String,
		msg_type: String,
		map: &mut HashMap<&str, String>,
	) -> Result<()> {
		let client = Client::new();

		//NOTE: previously we had to pass the payload as data and notification,
		// not sure anymore why, maybe its needed for android which we did not look into yet
		map.insert("Title", title.clone());
		map.insert("Body", body.clone());
		map.insert("msg-type", msg_type);

		let mut notification = NotificationBuilder::new();

		//NOTE: on ios this is used and shown
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
