#![forbid(unsafe_code)]
#![deny(
	dead_code,
	unused_imports,
	unused_must_use,
	unused_variables,
	unused_mut
)]
#![deny(clippy::all, clippy::pedantic, clippy::nursery)]
#![deny(
	clippy::as_conversions,
	clippy::dbg_macro,
	clippy::float_cmp_const,
	clippy::lossy_float_literal,
	clippy::string_to_string,
	clippy::unneeded_field_pattern,
	clippy::verbose_file_reads,
	clippy::unwrap_used,
	clippy::panic,
	clippy::needless_update,
	clippy::match_like_matches_macro,
	clippy::from_over_into,
	clippy::useless_conversion
)]
#![allow(clippy::module_name_repetitions)]

mod db;
mod error;
pub mod module;
pub mod resources;

use async_trait::async_trait;
use std::sync::Arc;

pub use db::InMemoryPubSub;
pub use db::RedisPubSub;

/// allows to query whether a specific user id is currently connected
//TODO: rename?
#[async_trait]
pub trait ConnectionState: Send + Sync {
	async fn update_status(&self, user_id: &str, connected: bool);
	async fn is_connected(&self, user_id: &str) -> bool;
}

/// `PubSubReceiver` will be sent messages to that arrived via the pubsub queue
#[async_trait]
pub trait PubSubReceiver: Send + Sync {
	async fn on_text(&self, topic: &str, payload: &str);
	async fn on_binary(&self, topic: &str, payload: Vec<u8>);
}

/// `PubSubSubcribable` allows for a lazy registration of a receiver of pubsub messages
#[async_trait]
pub trait PubSubSubcribable: Send + Sync {
	async fn subscribe(&self, receiver: Arc<dyn PubSubReceiver>);
}

/// `PubSubPublish` allows generic sending messages into the pubsub system
#[async_trait]
pub trait PubSubPublish: Send + Sync {
	async fn publish_text(&self, topic: &str, payload: &str);
	async fn publish_binary(&self, topic: &str, payload: Vec<u8>);
}

#[derive(Debug)]
pub enum RealtimeConnectionStatus {
	Connected,
	Disconnected,
}

/// `RealtimeReceive` allows to receive data send from the client.
/// `receive` will be called from a task spawned per receive.
#[async_trait]
//TODO: rename
pub trait RealtimeClientReceive: Send + Sync {
	async fn receive_text(&self, user_id: &str, payload: &str);
	async fn receive_binary(&self, user_id: &str, payload: &[u8]);
	async fn connection(
		&self,
		user_id: &str,
		status: RealtimeConnectionStatus,
	);
}
