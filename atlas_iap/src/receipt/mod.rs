pub mod dynamodb;
pub mod in_memory;

use crate::utc_time::UtcDateTime;

use super::error::Result;
use async_trait::async_trait;
use iap::UnityPurchaseReceipt;
use strum_macros::{EnumString, ToString};

#[derive(Debug, Copy, Clone, PartialEq, EnumString, ToString)]
pub enum Platform {
	GooglePlay,
	AppleAppStore,
}

impl From<iap::Platform> for Platform {
	fn from(v: iap::Platform) -> Self {
		match v {
			iap::Platform::GooglePlay => Self::GooglePlay,
			iap::Platform::AppleAppStore => Self::AppleAppStore,
		}
	}
}

#[derive(Debug, Clone, PartialEq)]
pub struct Receipt {
	pub transaction_id: String,
	pub platform: Platform,
	pub user_id: String,
	pub is_subscription: bool,
	pub product_id: String,
	pub unity_receipt: String,
	pub date: UtcDateTime,
}

impl Receipt {
	pub fn new(
		user_id: String,
		unity_receipt: &UnityPurchaseReceipt,
		is_subscription: bool,
		product_id: String,
		now: UtcDateTime,
		transaction_id: String,
	) -> Result<Self> {
		Ok(Self {
			transaction_id,
			platform: unity_receipt.store.clone().into(),
			user_id,
			is_subscription,
			product_id,
			unity_receipt: serde_json::to_string(&unity_receipt)?,
			date: now,
		})
	}

	pub fn get_data(&self) -> Result<UnityPurchaseReceipt> {
		Ok(serde_json::from_str(&self.unity_receipt)?)
	}
}

#[async_trait]
pub trait ReceiptDB: Send + Sync {
	async fn save_receipt(&self, r: Receipt) -> Result<()>;
	async fn get_receipt(
		&self,
		transaction_id: &str,
		platform: Platform,
	) -> Option<Receipt>;
}
