use rusoto_dynamodb::{DeleteItemError, PutItemError};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
	#[error("generic error: {0}")]
	Generic(String),
	#[error("unknown user error")]
	UnknownUser,
	#[error("unable to get entry")]
	GetEntry,
	#[error("dynamo error: {0}")]
	Atlas(#[from] atlasserver::error::Error),
	#[error("dynamo error: {0}")]
	Dyanamo(#[from] atlas_dynamo::Error),
	#[error("rusoto put error: {0}")]
	RusotoPutItem(#[from] rusoto_core::RusotoError<PutItemError>),
	#[error("rusoto delete item error: {0}")]
	RusotoDeleteItem(
		#[from] rusoto_core::RusotoError<DeleteItemError>,
	),
	#[error("siwa error: {0}")]
	Siwa(#[from] sign_in_with_apple::Error),
	#[error("fb error: {0}")]
	Fb(String),
}

pub type Result<T> = std::result::Result<T, Error>;
