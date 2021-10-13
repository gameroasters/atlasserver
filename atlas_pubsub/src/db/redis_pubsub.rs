use crate::{
	error::Result, ConnectionState, PubSubPublish, PubSubReceiver,
	PubSubSubcribable,
};
use async_trait::async_trait;
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use std::{sync::Arc, time::Duration};
use tokio::sync::RwLock;
use tokio_stream::StreamExt;
use tracing::instrument;

/// helper type to wrap text or binary messages into a shared type
/// we use binary serialization of this type to send in a unified way via redis pubsub
#[derive(Serialize, Deserialize)]
enum Msg {
	Text(String),
	Binary(Vec<u8>),
}

const CONNECTION_STATE_TTL_SECONDS: usize = 10;

#[derive(Clone)]
pub struct RedisPubSub {
	receiver: Arc<RwLock<Option<Arc<dyn PubSubReceiver>>>>,
	url: String,
	redis: deadpool_redis::Pool,
}

impl RedisPubSub {
	#[allow(clippy::missing_errors_doc)]
	pub async fn new(
		pool: deadpool_redis::Pool,
		url: String,
	) -> Result<Self> {
		let new = Self {
			receiver: Arc::new(RwLock::new(None)),
			url,
			redis: pool,
		};

		let new_res = new.clone();

		tokio::spawn(async move {
			let sub = new.clone();
			loop {
				if let Err(e) = sub.subscriber_task().await {
					tracing::error!("subscriber err: {}", e);
				}

				tokio::time::sleep(Duration::from_secs(2)).await;
			}
		});

		Ok(new_res)
	}

	#[instrument(skip(self), err)]
	async fn subscriber_task(&self) -> Result<()> {
		tracing::info!("subscriber_task");

		let client = redis::Client::open(self.url.clone())?;
		let client = client.get_tokio_connection().await?;

		let mut pubsub = client.into_pubsub();
		pubsub.psubscribe("atlas/*").await?;

		tracing::info!("subscribed");

		let mut pubsub_stream = pubsub.on_message();

		while let Some(msg) = pubsub_stream.next().await {
			let payload = msg.get_payload_bytes();
			let topic = msg.get_channel_name();

			tracing::info!(target: "received", bytes = payload.len(), topic = ?topic);

			if let Some(topic) = topic.strip_prefix("atlas/") {
				self.forward(topic, payload).await;
			}
		}

		Ok(())
	}

	#[instrument(skip(self, payload))]
	async fn forward(&self, topic: &str, payload: &[u8]) {
		tracing::info!("forward: {}b", payload.len());

		if let Ok(msg) = postcard::from_bytes::<Msg>(payload) {
			let receiver = self.receiver.read().await;
			if let Some(receiver) = receiver.as_ref() {
				match msg {
					Msg::Binary(buffer) => {
						receiver.on_binary(topic, buffer).await;
					}
					Msg::Text(text) => {
						receiver.on_text(topic, &text).await;
					}
				}
			}
		}
	}

	async fn publish_msg(&self, topic: &str, msg: Msg) {
		if let Ok(msg) = postcard::to_stdvec(&msg) {
			if let Ok(mut db) = self.redis.get().await {
				if let Err(e) = db
					.publish::<_, _, ()>(
						format!("atlas/{}", topic),
						msg,
					)
					.await
				{
					tracing::error!("publish err: {}", e);
				}
			}
		}
	}
}

#[async_trait]
impl PubSubSubcribable for RedisPubSub {
	async fn subscribe(&self, receiver: Arc<dyn PubSubReceiver>) {
		let mut r = self.receiver.write().await;
		*r = Some(receiver);
	}
}

#[async_trait]
impl PubSubPublish for RedisPubSub {
	async fn publish_text(&self, topic: &str, payload: &str) {
		let msg = Msg::Text(payload.to_string());
		self.publish_msg(topic, msg).await;
	}

	async fn publish_binary(&self, topic: &str, payload: Vec<u8>) {
		let msg = Msg::Binary(payload);
		self.publish_msg(topic, msg).await;
	}
}

#[async_trait]
impl ConnectionState for RedisPubSub {
	async fn update_status(&self, user_id: &str, connected: bool) {
		let key = format!("atlas/connected/{}", user_id);

		if let Ok(mut db) = self.redis.get().await {
			if connected {
				db.set_ex::<String, String, String>(
					key,
					String::new(),
					CONNECTION_STATE_TTL_SECONDS,
				)
				.await
				.ok();
			} else {
				db.del::<String, String>(key).await.ok();
			}
		}
	}

	async fn is_connected(&self, user_id: &str) -> bool {
		let key = format!("atlas/connected/{}", user_id);

		if let Ok(mut db) = self.redis.get().await {
			db.get::<String, String>(key).await.ok().is_some()
		} else {
			false
		}
	}
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
#[allow(clippy::panic)]
mod test {
	use super::*;
	use pretty_assertions::assert_eq;
	use std::iter;

	#[test]
	fn test_length() {
		let payload: Vec<u8> =
			iter::repeat(1).take(1_000_000).collect();

		let msg = Msg::Binary(payload.clone());
		let buffer = postcard::to_stdvec(&msg).unwrap();

		let msg2 = postcard::from_bytes::<Msg>(&buffer).unwrap();

		if let Msg::Binary(payload2) = &msg2 {
			assert_eq!(payload2, &payload);
		} else {
			panic!("wrong msg");
		}
	}
}
