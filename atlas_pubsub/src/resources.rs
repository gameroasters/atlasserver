use super::{PubSubPublish, PubSubReceiver, PubSubSubcribable};
use crate::{
	ConnectionState, RealtimeClientReceive, RealtimeConnectionStatus,
};
use async_trait::async_trait;
use futures::StreamExt;
use futures_util::FutureExt;
use std::{
	collections::HashMap,
	sync::{
		atomic::{AtomicBool, Ordering},
		Arc,
	},
};
use tokio::sync::{mpsc, RwLock};
use tokio_stream::wrappers::UnboundedReceiverStream;
use tracing::instrument;
use warp::ws::{Message, WebSocket};

type OutBoundChannel =
	mpsc::UnboundedSender<std::result::Result<Message, warp::Error>>;

/// here we handle incoming websocket connections (see `on_connect`)
/// and register into one single topic.
pub struct PubSubSubscriberResource {
	shutdown: AtomicBool,
	connection_states: Arc<dyn ConnectionState>,
	client_channels: Arc<RwLock<HashMap<String, OutBoundChannel>>>,
	client_receive:
		Arc<RwLock<Option<Arc<dyn RealtimeClientReceive>>>>,
}

impl PubSubSubscriberResource {
	pub async fn new(
		connection_states: Arc<dyn ConnectionState>,
		pubsub: Arc<dyn PubSubSubcribable>,
	) -> Arc<Self> {
		let new = Arc::new(Self {
			connection_states,
			client_channels: Arc::<
				RwLock<HashMap<String, OutBoundChannel>>,
			>::default(),
			client_receive: Arc::new(RwLock::new(None)),
			shutdown: AtomicBool::new(false),
		});

		pubsub.subscribe(new.clone()).await;

		new
	}

	pub fn shutdown(&self) {
		self.shutdown
			.store(true, std::sync::atomic::Ordering::Relaxed);
	}

	pub async fn count_connections(&self) -> usize {
		self.client_channels.read().await.len()
	}

	pub async fn set_receiver(
		&self,
		receiver: Arc<dyn RealtimeClientReceive>,
	) {
		let mut r = self.client_receive.write().await;
		*r = Some(receiver);
	}

	///
	#[instrument(skip(self, ws))]
	pub async fn on_connect(&self, user_id: &str, ws: WebSocket) {
		tracing::info!("[pubsub] welcome user");

		let (ws_send, mut ws_receive) = ws.split();

		let (send, receiver) = mpsc::unbounded_channel();
		let receiver = UnboundedReceiverStream::new(receiver);

		//TODO: is this correctly deconstructed?
		tokio::task::spawn(receiver.forward(ws_send).map(|result| {
			if let Err(e) = result {
				tracing::error!("websocket send error: {}", e);
			}
		}));

		self.client_channels
			.write()
			.await
			.insert(String::from(user_id), send.clone());

		self.forward_connection_status(
			user_id,
			RealtimeConnectionStatus::Connected,
		)
		.await;

		while let Some(result) = ws_receive.next().await {
			match result {
				Ok(msg) if msg.is_ping() => {
					tracing::trace!("got ping");

					self.connection_states
						.update_status(user_id, true)
						.await;
				}
				Ok(msg) if msg.is_close() => {
					tracing::trace!("got close");
					break;
				}
				Ok(msg) => {
					self.client_receive(user_id, msg).await;
				}
				Err(e) => {
					tracing::error!("websocket err: '{}'", e);
					break;
				}
			};

			if self.shutdown.load(Ordering::Relaxed) {
				tracing::info!("[pubsub] close client connection");

				send.send(Ok(Message::close_with(
					4000_u16,
					"server shutdown",
				)))
				.ok();

				break;
			}
		}

		tracing::info!("[pubsub] bye user");

		self.client_channels.write().await.remove(user_id);

		self.forward_connection_status(
			user_id,
			RealtimeConnectionStatus::Disconnected,
		)
		.await;
	}

	#[instrument(skip(self, msg))]
	async fn send_msg(&self, topic: &str, msg: Message) {
		let clients = self.client_channels.clone();
		let topic = String::from(topic);

		tokio::spawn(async move {
			let clients = clients.read().await;
			if let Some(client) = clients.get(&topic) {
				if let Err(e) = client.send(Ok(msg)) {
					tracing::error!("[pubsub] send error: {}", e);
				}
			}
		});
	}

	#[instrument(skip(self, msg))]
	async fn client_receive(&self, user_id: &str, msg: Message) {
		tracing::info!("client_receive");

		let receiver = self.client_receive.read().await.clone();
		if let Some(receiver) = receiver {
			let user_id = String::from(user_id);
			tokio::spawn(async move {
				if msg.is_text() {
					match msg.to_str() {
						Ok(msg) => {
							receiver
								.receive_text(&user_id, msg)
								.await;
						}
						Err(_) => {
							tracing::error!("msg text error",);
						}
					}
				} else if msg.is_binary() {
					receiver
						.receive_binary(&user_id, msg.as_bytes())
						.await;
				}
			});
		}
	}

	#[instrument(skip(self))]
	async fn forward_connection_status(
		&self,
		user_id: &str,
		status: RealtimeConnectionStatus,
	) {
		tracing::info!("client_connection");

		self.connection_states
			.update_status(
				user_id,
				matches!(status, RealtimeConnectionStatus::Connected),
			)
			.await;

		let receiver = self.client_receive.read().await.clone();
		if let Some(receiver) = receiver {
			let user_id = String::from(user_id);
			tokio::spawn(async move {
				receiver.connection(&user_id, status).await;
			});
		}
	}
}

#[async_trait]
impl PubSubReceiver for PubSubSubscriberResource {
	#[instrument(skip(self))]
	async fn on_text(&self, topic: &str, payload: &str) {
		tracing::info!("on_text");
		let payload = String::from(payload);
		self.send_msg(topic, Message::text(payload)).await;
	}

	#[instrument(skip(self, payload))]
	async fn on_binary(&self, topic: &str, payload: Vec<u8>) {
		tracing::info!("on_binary");
		self.send_msg(topic, Message::binary(payload)).await;
	}
}

//TODO: can we go by without this resource and just put the trait into our resources?
pub struct PubSubPublishResource {
	publish: Arc<dyn PubSubPublish>,
}

impl PubSubPublishResource {
	#[must_use]
	pub fn new(publish: Arc<dyn PubSubPublish>) -> Self {
		Self { publish }
	}
}

#[async_trait]
impl PubSubPublish for PubSubPublishResource {
	///
	#[instrument(skip(self))]
	async fn publish_text(&self, topic: &str, payload: &str) {
		tracing::debug!("[pubsub] publish_text");
		self.publish.publish_text(topic, payload).await;
	}
	///
	#[instrument(skip(self, payload))]
	async fn publish_binary(&self, topic: &str, payload: Vec<u8>) {
		tracing::debug!("[pubsub] publish_binary");
		self.publish.publish_binary(topic, payload).await;
	}
}
