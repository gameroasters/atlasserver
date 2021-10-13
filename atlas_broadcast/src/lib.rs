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

use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::instrument;

#[async_trait]
pub trait BroadcastReceiver<T: Clone + Send + Sync>:
	Send + Sync
{
	async fn receive(&self, msg: T);
}

pub struct BroadcastResource<T: Clone + Send + Sync> {
	sender: broadcast::Sender<T>,
}

impl<T: Clone + Send + Sync + 'static> BroadcastResource<T> {
	#[must_use]
	pub fn new(capacity: usize) -> Self {
		let (sender, _) = broadcast::channel(capacity);
		Self { sender }
	}

	#[instrument(skip(self, subscriber))]
	pub async fn subscribe(
		&self,
		subscriber: Arc<dyn BroadcastReceiver<T>>,
	) {
		let receiver = self.sender.subscribe();

		tracing::info!(
			"receiver added: {}",
			self.sender.receiver_count()
		);

		tokio::spawn(async move {
			let mut receiver = receiver;

			loop {
				match receiver.recv().await {
					Ok(msg) => subscriber.receive(msg).await,
					Err(e) => tracing::error!("receive error: {}", e),
				}
			}
		});
	}

	pub fn publish(&self, msg: T) {
		if let Err(e) = self.sender.send(msg) {
			tracing::error!("send error: {}", e);
		}
	}
}

#[cfg(test)]
mod test {
	#![allow(clippy::semicolon_if_nothing_returned)]

	use super::*;
	use std::{
		sync::atomic::{AtomicU64, Ordering},
		time::Duration,
	};
	use tokio::time::sleep;

	#[derive(Debug, Clone)]
	struct TestMessageType(u64);

	struct TestReceiver {
		count: AtomicU64,
	}
	impl Default for TestReceiver {
		fn default() -> Self {
			Self {
				count: AtomicU64::new(0),
			}
		}
	}
	#[async_trait]
	impl BroadcastReceiver<TestMessageType> for TestReceiver {
		async fn receive(&self, msg: TestMessageType) {
			self.count.fetch_add(
				msg.0,
				std::sync::atomic::Ordering::SeqCst,
			);
		}
	}

	#[tokio::test]
	async fn test_smoke() {
		let res = Arc::new(BroadcastResource::new(1));
		res.publish(TestMessageType(0));

		let receiver = Arc::new(TestReceiver::default());
		res.subscribe(receiver.clone()).await;

		res.publish(TestMessageType(1));

		sleep(Duration::from_millis(10)).await;
		assert_eq!(receiver.count.load(Ordering::SeqCst), 1);

		{
			let res2 = res.clone();
			res2.publish(TestMessageType(2));
		}

		sleep(Duration::from_millis(10)).await;
		assert_eq!(receiver.count.load(Ordering::SeqCst), 3);
	}
}
