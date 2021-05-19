#![allow(clippy::pub_enum_variant_names)]

use rusoto_core::{
	credential::CredentialsError, request::TlsError, RusotoError,
};
use rusoto_dynamodb::{
	CreateTableError, ListTablesError, PutItemError,
};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
	#[error("io error: {0}")]
	IoError(#[from] std::io::Error),

	#[error("aws error: {0}")]
	RusotoPutItemError(#[from] RusotoError<PutItemError>),

	#[error("table {0} not found error")]
	TableNotFoundError(String),

	#[error("aws error: {0}")]
	RusotoListTablesError(#[from] RusotoError<ListTablesError>),

	#[error("aws error: {0}")]
	RusotoCreateTableError(#[from] RusotoError<CreateTableError>),

	#[error("aws error: {0}")]
	RusotoCredentialsError(#[from] CredentialsError),

	#[error("aws error: {0}")]
	RusotoTlsError(#[from] TlsError),

	#[error("DynamoDeserializeError for field: {0}")]
	DynamoDeserializeError(&'static str),

	#[error("custom error: {0}")]
	Custom(String),
}

pub type Result<T> = std::result::Result<T, Error>;
