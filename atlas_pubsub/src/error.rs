use redis::RedisError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
	#[error("dynamo error: {0}")]
	Atlas(#[from] atlasserver::error::Error),

	#[error("redis error: {0}")]
	Redis(#[from] RedisError),
}

pub type Result<T> = std::result::Result<T, Error>;
