use crate::schema;
#[cfg(feature = "json-proto")]
use serde::{de::DeserializeOwned, Serialize};
use warp::{
	body::aggregate,
	http::HeaderValue,
	hyper::{header::CONTENT_TYPE, StatusCode},
	reject::{self, Reject},
	reply::Response,
	Buf, Filter, Rejection, Reply,
};

#[derive(Debug)]
struct ProtobufDeseralizeError {
	cause: Box<dyn std::error::Error + Send + Sync>,
}

impl Reject for ProtobufDeseralizeError {}

#[cfg(feature = "json-proto")]
pub fn protobuf_body<
	T: schema::Message + Send + Default + DeserializeOwned,
>() -> impl Filter<Extract = (T,), Error = Rejection> + Copy {
	async fn from_bytes<
		T: schema::Message + Send + Default + DeserializeOwned,
	>(
		mut buf: impl Buf + Send,
		content_type: Option<String>,
	) -> Result<T, Rejection> {
		let bytes = buf.copy_to_bytes(buf.remaining());

		match content_type {
			Some(h) if &h == "application/json" => {
				serde_json::from_slice(&bytes.to_vec()).map_err(
					|err| {
						tracing::debug!(
							"json request protobuf body error: {}",
							err
						);
						ProtobufDeseralizeError { cause: err.into() }
					},
				)
			}
			_ => T::parse_from_bytes(&bytes).map_err(|err| {
				ProtobufDeseralizeError { cause: err.into() }
			}),
		}
		.map_err(reject::custom)
	}
	aggregate()
		.and(warp::header::optional("x-content-type"))
		.and_then(from_bytes)
}

#[cfg(not(feature = "json-proto"))]
pub fn protobuf_body<T: schema::Message + Send + Default>(
) -> impl Filter<Extract = (T,), Error = Rejection> + Copy {
	async fn from_bytes<T: schema::Message + Send + Default>(
		mut buf: impl Buf + Send,
	) -> Result<T, Rejection> {
		let bytes = buf.copy_to_bytes(buf.remaining());

		match T::parse_from_bytes(&bytes) {
			Ok(res) => Ok(res),
			Err(err) => {
				tracing::debug!(
					"failed to parse protobuf object: {}",
					err
				);

				Err(reject::custom(ProtobufDeseralizeError {
					cause: err.into(),
				}))
			}
		}
	}
	aggregate().and_then(from_bytes)
}

pub struct Protobuf {
	inner: Result<Vec<u8>, ()>,
}

impl Reply for Protobuf {
	fn into_response(self) -> Response {
		match self.inner {
			Ok(body) => {
				let mut res = Response::new(body.into());
				res.headers_mut().insert(
					CONTENT_TYPE,
					HeaderValue::from_static(
						"application/x-protobuf",
					),
				);
				res
			}
			Err(()) => {
				StatusCode::INTERNAL_SERVER_ERROR.into_response()
			}
		}
	}
}

#[cfg(not(feature = "json-proto"))]
pub fn protobuf_reply<T>(val: &T) -> Protobuf
where
	T: schema::Message + Send + Default,
{
	Protobuf {
		inner: val.write_to_bytes().map_err(|err| {
			tracing::debug!("protobuf reply error: {}", err)
		}),
	}
}

#[cfg(feature = "json-proto")]
pub fn protobuf_reply<T>(
	val: &T,
	content_type: Option<String>,
) -> Protobuf
where
	T: schema::Message + Send + Default + Serialize,
{
	Protobuf {
		inner: match content_type {
			Some(t) if &t == "application/json" => {
				serde_json::to_vec(&val).map_err(|err| {
					tracing::debug!("json reply error: {}", err)
				})
			}
			_ => val.write_to_bytes().map_err(|err| {
				tracing::debug!("protobuf reply error: {}", err)
			}),
		},
	}
}
