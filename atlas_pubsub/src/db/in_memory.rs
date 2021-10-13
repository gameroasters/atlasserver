use async_trait::async_trait;
use std::{collections::HashSet, sync::Arc};
use tokio::sync::RwLock;

use crate::{
	ConnectionState, PubSubPublish, PubSubReceiver, PubSubSubcribable,
};

#[derive(Default)]
pub struct InMemoryPubSub {
	receiver: Arc<RwLock<Option<Arc<dyn PubSubReceiver>>>>,
	connected: Arc<RwLock<HashSet<String>>>,
}

#[async_trait]
impl PubSubSubcribable for InMemoryPubSub {
	async fn subscribe(&self, receiver: Arc<dyn PubSubReceiver>) {
		let mut r = self.receiver.write().await;
		*r = Some(receiver);
	}
}

#[async_trait]
impl PubSubPublish for InMemoryPubSub {
	async fn publish_text(&self, topic: &str, payload: &str) {
		let receiver = self.receiver.read().await;
		if let Some(receiver) = receiver.as_ref() {
			receiver.on_text(topic, payload).await;
		}
	}

	async fn publish_binary(&self, topic: &str, payload: Vec<u8>) {
		let receiver = self.receiver.read().await;
		if let Some(receiver) = receiver.as_ref() {
			receiver.on_binary(topic, payload).await;
		}
	}
}

#[async_trait]
impl ConnectionState for InMemoryPubSub {
	async fn update_status(&self, user_id: &str, connected: bool) {
		let mut connections = self.connected.write().await;

		if connected {
			connections.insert(user_id.into());
		} else {
			connections.remove(user_id);
		}
	}

	async fn is_connected(&self, user_id: &str) -> bool {
		let connections = self.connected.read().await;
		connections.contains(user_id)
	}
}
