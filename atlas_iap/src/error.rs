use rusoto_dynamodb::PutItemError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
	#[error("iap error: {0}")]
	Iap(#[from] iap::error::Error),
	#[error("serde_json error: {0}")]
	Json(#[from] serde_json::Error),
	#[error("dynamo error: {0}")]
	Atlas(#[from] atlas_dynamo::Error),
	#[error("rusoto put error: {0}")]
	RusotoPutItem(#[from] rusoto_core::RusotoError<PutItemError>),
	#[error("DynamoDeserializeError for field: {0}")]
	DynamoDeserialize(&'static str),
	#[error("custom error: {0}")]
	Custom(String),
}

pub type Result<T> = std::result::Result<T, Error>;
