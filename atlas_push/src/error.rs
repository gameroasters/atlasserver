use rusoto_dynamodb::PutItemError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
	#[error("serde json error {0}")]
	SerdeJson(#[from] serde_json::Error),
	#[error("fcm error: {0}")]
	Fcm(#[from] fcm::FcmError),
	#[error("dynamo error: {0}")]
	Atlas(#[from] atlasserver::error::Error),
	#[error("rusoto put error: {0}")]
	DynamoPut(#[from] rusoto_core::RusotoError<PutItemError>),
}

pub type Result<T> = std::result::Result<T, Error>;
