use rusoto_core::{
	credential::CredentialsError, request::TlsError, RusotoError,
};
use rusoto_dynamodb::{
	CreateTableError, ListTablesError, PutItemError,
};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
	#[error("custom error: {0}")]
	Custom(String),

	#[error("io error: {0}")]
	Io(#[from] std::io::Error),

	#[error("aws error: {0}")]
	RusotoPutItem(#[from] RusotoError<PutItemError>),

	#[error("table {0} not found error")]
	TableNotFound(String),

	#[error("aws error: {0}")]
	RusotoListTables(#[from] RusotoError<ListTablesError>),

	#[error("aws error: {0}")]
	RusotoCreateTable(#[from] RusotoError<CreateTableError>),

	#[error("aws error: {0}")]
	RusotoCredentials(#[from] CredentialsError),

	#[error("aws error: {0}")]
	RusotoTls(#[from] TlsError),

	#[error("DynamoDeserializeError for field: {0}")]
	DynamoDeserialize(&'static str),
}

pub type Result<T> = std::result::Result<T, Error>;
