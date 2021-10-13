use crate::{
	error::Result,
	receipt::{Platform, ReceiptDB},
	schema,
	utc_time::UtcDateTime,
	IapEventHandler, Receipt,
};
use iap::{
	PurchaseResponse, ReceiptValidator, SkuType,
	UnityPurchaseReceipt, UnityPurchaseValidator,
};
use std::sync::Arc;
use tracing::instrument;

pub struct IapResource {
	validator: Arc<dyn ReceiptValidator>,
	receipts: Arc<dyn ReceiptDB>,
	event_handler: Option<Arc<dyn IapEventHandler>>,
}

pub struct ResponseData {
	response: PurchaseResponse,
	environment: Option<String>,
	is_subscription: bool,
	transaction_id: String,
}

impl IapResource {
	pub fn new(
		receipts: Arc<dyn ReceiptDB>,
		apple_secret: Option<String>,
		google_secret: Option<String>,
	) -> Result<Self> {
		tracing::info!(
			"iap secrets: (apple: {}, google: {})",
			apple_secret.is_some(),
			google_secret.is_some()
		);

		let mut validator = UnityPurchaseValidator::default();
		if let Some(apple_secret) = apple_secret {
			validator = validator.set_apple_secret(apple_secret);
		}
		if let Some(google_secret) = google_secret {
			validator = validator
				.set_google_service_account_key(google_secret)?;
		}

		Ok(Self {
			receipts,
			validator: Arc::new(validator),
			event_handler: None,
		})
	}

	#[must_use]
	pub fn from_validator(
		receipts: Arc<dyn ReceiptDB>,
		validator: Arc<dyn ReceiptValidator>,
	) -> Self {
		Self {
			receipts,
			validator,
			event_handler: None,
		}
	}

	pub fn set_event_handler(
		&mut self,
		event: Arc<dyn IapEventHandler>,
	) {
		self.event_handler = Some(event);
	}

	pub async fn get_receipt(
		&self,
		transaction_id: &str,
		platform: Platform,
	) -> Option<Receipt> {
		self.receipts.get_receipt(transaction_id, platform).await
	}

	#[instrument(skip(self, receipt), err)]
	pub async fn validate_purchase(
		&self,
		user_id: &str,
		receipt: &UnityPurchaseReceipt,
		now: UtcDateTime,
	) -> Result<schema::PurchaseResponse> {
		tracing::info!(target: "iap-receipt",
			user = %&user_id,
			?receipt,
		);

		let ResponseData {
			response,
			environment,
			is_subscription,
			transaction_id,
		} = match receipt.store {
			iap::Platform::AppleAppStore => {
				self.validate_apple(receipt, now).await?
			}
			iap::Platform::GooglePlay => {
				self.validate_google(receipt, now).await?
			}
		};

		let environment = environment.unwrap_or_default();

		tracing::info!(target: "iap",
			user = %&user_id,
			platform = ?&receipt.store,
			%environment,
			transaction_id = %transaction_id,
			valid = %&response.valid,
			product_id = ?&response.product_id
		);

		if response.valid {
			let receipt = Receipt::new(
				user_id.to_string(),
				receipt,
				is_subscription,
				//TODO: we should fail when product_id is None, can't insert empty strings to db
				response.product_id.unwrap_or_default(),
				now,
				transaction_id.clone(),
			)?;

			if !is_subscription
				&& self
					.receipts
					.get_receipt(&transaction_id, receipt.platform)
					.await
					.is_some()
			{
				tracing::error!(
					"This product has already been consumed"
				);
				//TODO: extend this to include the reason for invalidity if we need it in the client
				return Ok(schema::PurchaseResponse {
					valid: false,
					..schema::PurchaseResponse::default()
				});
			}

			if let Some(handler) = self.event_handler.as_ref() {
				handler.on_valid_receipt(&receipt).await?;
			}

			self.receipts.save_receipt(receipt.clone()).await?;
		}

		//TODO: impl From if this ever gets more complicated
		Ok(schema::PurchaseResponse {
			valid: response.valid,
			..schema::PurchaseResponse::default()
		})
	}

	async fn validate_google(
		&self,
		receipt: &UnityPurchaseReceipt,
		now: UtcDateTime,
	) -> Result<ResponseData> {
		let (response, sku_type) =
			self.validator.fetch_google_receipt_data(receipt).await?;
		let (purchase_response, is_subscription) = match sku_type {
			SkuType::Subs => (
				iap::validate_google_subscription(&response, now)?,
				true,
			),
			SkuType::Inapp => {
				(iap::validate_google_package(&response), false)
			}
		};
		let environment =
			Some(String::from(response.purchase_type.map_or(
				"Production",
				|purchase_type| {
					if purchase_type == 0 {
						"Test"
					} else {
						"Promo"
					}
				},
			)));
		Ok(ResponseData {
			response: purchase_response,
			environment,
			is_subscription,
			transaction_id: receipt.transaction_id.clone(),
		})
	}

	async fn validate_apple(
		&self,
		receipt: &UnityPurchaseReceipt,
		now: UtcDateTime,
	) -> Result<ResponseData> {
		let response =
			self.validator.fetch_apple_receipt_data(receipt).await?;
		let is_subscription =
			response.is_subscription(&receipt.transaction_id);
		let purchase_response = if is_subscription {
			iap::validate_apple_subscription(
				&response,
				&receipt.transaction_id,
				now,
			)
		} else {
			iap::validate_apple_package(
				&response,
				&receipt.transaction_id,
			)
		};

		let transaction_id = if receipt.transaction_id.is_empty() {
			// TODO: maybe just return the transaction_id in PurchaseResponse the same way
			// we do for product_id to avoid having to get the receipt more than once
			response
				.get_latest_receipt()
				.and_then(|receipt| receipt.transaction_id)
				.ok_or_else(|| {
					crate::error::Error::Custom(String::from(
						"Failed to get transaction id",
					))
				})?
		} else {
			receipt.transaction_id.clone()
		};

		let environment = response.environment;

		Ok(ResponseData {
			response: purchase_response,
			environment,
			is_subscription,
			transaction_id,
		})
	}
}

#[cfg(test)]
mod tests {
	#![allow(
		clippy::unwrap_used,
		clippy::default_trait_access,
		clippy::panic,
        //TODO: https://github.com/rust-lang/rust-clippy/issues/7438
		clippy::semicolon_if_nothing_returned
	)]

	use super::IapResource;
	use crate::{
		receipt::Receipt, utc_time::UtcDateTime, IapEventHandler,
		InMemoryReceiptDB,
	};
	use async_trait::async_trait;
	use chrono::{Duration, Utc};
	use iap::{
		error::Result, AppleResponse, GoogleResponse,
		PurchaseResponse, ReceiptDataFetcher, ReceiptValidator,
		SkuType, UnityPurchaseReceipt, Validator,
	};
	use mockall::predicate;
	use std::sync::{atomic::AtomicBool, Arc};

	mockall::mock! {
		pub UnityPurchaseValidator {}
		#[async_trait]
		impl ReceiptDataFetcher for UnityPurchaseValidator {
			async fn fetch_apple_receipt_data(&self, receipt: &UnityPurchaseReceipt) -> Result<AppleResponse>;
			async fn fetch_google_receipt_data(&self, receipt: &UnityPurchaseReceipt) -> Result<(GoogleResponse, SkuType)>;
		}
		#[async_trait]
		impl Validator for UnityPurchaseValidator {
			async fn validate(&self, now: UtcDateTime, receipt: &UnityPurchaseReceipt) -> Result<PurchaseResponse>;
		}
		impl ReceiptValidator for UnityPurchaseValidator{}
	}

	mockall::mock! {
		pub MyIapEventHandler {}
		#[async_trait]
		impl IapEventHandler for MyIapEventHandler {
			async fn on_valid_receipt(&self, receipt: &Receipt) -> std::result::Result<(), crate::error::Error>;
		}
	}

	#[tokio::test]
	async fn test_handler_called() {
		let now = Utc::now();

		let unity_receipt = UnityPurchaseReceipt {
			store: iap::Platform::AppleAppStore,
			payload: "foo".to_string(),
			transaction_id: "bar".to_string(),
		};

		let _receipt = Receipt::new(
			"uid".to_string(),
			&unity_receipt.clone(),
			true,
			"prod".to_string(),
			now,
			"bar".to_string(),
		);

		let was_called = Arc::new(AtomicBool::new(false));

		let called_clone = was_called.clone();
		let receipt = unity_receipt.clone();
		let mut mock_event_handler = MockMyIapEventHandler::new();
		mock_event_handler
			.expect_on_valid_receipt()
			.with(predicate::function(move |x: &Receipt| {
				let unity_receipt = x.get_data().unwrap();
				unity_receipt.payload == receipt.payload
					&& &x.user_id == "uid"
			}))
			.returning(move |_| {
				called_clone
					.store(true, std::sync::atomic::Ordering::SeqCst);
				Ok(())
			});

		let mut mock_validator = MockUnityPurchaseValidator::new();
		mock_validator
			.expect_fetch_apple_receipt_data()
			.with(predicate::always())
			.returning(move |_| {
				let json_string = format!(
					r#"{{
						"status": 0,
						"latest_receipt":"foo",
						"latest_receipt_info":[ 
							{{
								"quantity": "1",
								"expires_date_ms": "{}",
								"expires_date": "",
								"original_purchase_date": "",
								"product_id": "prod",
								"purchase_date": "",
								"transaction_id": "bar"
							}}
						],
                        "receipt": {{
                            "in_app": [{{
                                "transaction_id": "bar",
                                "product_id": "prod"
                            }}]
                        }}
					}}"#,
					(now + Duration::days(1)).timestamp_millis()
				);

				Ok(serde_json::from_str::<AppleResponse>(
					&json_string,
				)
				.unwrap())
			});

		let mock_event_handler = Arc::new(mock_event_handler);

		mock_validator
			.expect_validate()
			.with(predicate::always(), predicate::always())
			.returning(|_, _| {
				Ok(PurchaseResponse {
					valid: true,
					product_id: None,
				})
			});

		let resource = IapResource {
			receipts: Arc::new(InMemoryReceiptDB::default()),
			validator: Arc::new(mock_validator),
			event_handler: Some(mock_event_handler.clone()),
		};

		resource
			.validate_purchase("uid", &unity_receipt.clone(), now)
			.await
			.unwrap();

		assert!(was_called.load(std::sync::atomic::Ordering::SeqCst));

		// test whether it will allow redos of consumed products
		was_called.store(false, std::sync::atomic::Ordering::SeqCst);
		resource
			.validate_purchase("uid", &unity_receipt, now)
			.await
			.unwrap();

		assert!(!was_called.load(std::sync::atomic::Ordering::SeqCst));
	}
}
