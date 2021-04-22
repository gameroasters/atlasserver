use crate::schema;
#[cfg(feature = "json-proto")]
use serde::de::DeserializeOwned;
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
		mut buf: impl Buf,
	) -> Result<T, Rejection> {
		let bytes = buf.copy_to_bytes(buf.remaining());

		match T::parse_from_bytes(&bytes) {
			Ok(res) => Ok(res),
			Err(err) => {
				log::debug!("json fallback due to request protobuf body error: {}", err);

				serde_json::from_slice(&bytes.to_vec()).map_err(
					|err| {
						log::debug!(
							"json request protobuf body error: {}",
							err
						);
						reject::custom(ProtobufDeseralizeError {
							cause: err.into(),
						})
					},
				)
			}
		}
	}
	aggregate().and_then(from_bytes)
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
