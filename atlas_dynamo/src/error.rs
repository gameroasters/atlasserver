use rusoto_dynamodb::{
	CreateTableError, DeleteItemError, ListTablesError, PutItemError,
};
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
}

pub type Result<T> = std::result::Result<T, Error>;
