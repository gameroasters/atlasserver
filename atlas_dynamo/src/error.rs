use rusoto_core::{
	credential::CredentialsError, request::TlsError, RusotoError,
};
use rusoto_dynamodb::{
	CreateTableError, DeleteItemError, ListTablesError, PutItemError,
};
use rusoto_secretsmanager::GetSecretValueError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
	#[error("generic error: {0}")]
	Generic(String),
	#[error("unknown user error")]
	UnknownUser,
	#[error("unable to get entry")]
	GetEntry,
	#[error("rusoto error: {0}")]
	RusotoCreateTable(
		#[from] rusoto_core::RusotoError<CreateTableError>,
	),
	#[error("rusoto put error: {0}")]
	RusotoPutItem(#[from] rusoto_core::RusotoError<PutItemError>),
	#[error("rusoto put error: {0}")]
	RusotoListTables(
		#[from] rusoto_core::RusotoError<ListTablesError>,
	),
	#[error("rusoto delete item error: {0}")]
	RusotoDeleteItem(
		#[from] rusoto_core::RusotoError<DeleteItemError>,
	),
	#[error("aws error: {0}")]
	RusotoCredentials(#[from] CredentialsError),
	#[error("aws error: {0}")]
	RusotoTls(#[from] TlsError),
	#[error("table {0} not found error")]
	TableNotFound(String),
	#[error("DynamoDeserializeError for field: {0}")]
	DynamoDeserialize(&'static str),
	#[error("rusoto secret general error: {0}")]
	RusotoSecret(String),
	#[error("rusoto secret get value error: {0}")]
	RusotoGetSecretValue(#[from] RusotoError<GetSecretValueError>),
}

pub type Result<T> = std::result::Result<T, Error>;
