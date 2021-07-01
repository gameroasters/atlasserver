use crate::schema::{self, RejectionResponse};
use std::convert::Infallible;
use warp::{hyper::StatusCode, reject::Reject, Rejection, Reply};

#[derive(Debug)]
pub enum SessionFailure {
	Invalid,
	SessionNotFound,
}

impl Reject for SessionFailure {}

#[allow(clippy::missing_errors_doc)]
pub async fn handle_rejection(
	err: Rejection,
) -> Result<impl Reply, Infallible> {
	err.find::<SessionFailure>().map_or_else(
        || {
			tracing::error!("unhandled rejection {:?}", err);

            Ok(warp::reply::with_status(
                crate::pbwarp::protobuf_reply(&RejectionResponse::default(), None),
                StatusCode::INTERNAL_SERVER_ERROR,
            ))
        },
        |session_failure| {
            let mut rejection = schema::RejectionResponse::default();

            match session_failure {
                SessionFailure::Invalid => rejection.set_sessionFilterRejection(
                    schema::RejectionResponse_SessionFilterRejection::INVALID,
                ),
                SessionFailure::SessionNotFound => rejection.set_sessionFilterRejection(
                    schema::RejectionResponse_SessionFilterRejection::SESSION_NOT_FOUND,
                ),
            };

            Ok(warp::reply::with_status(
                crate::pbwarp::protobuf_reply(&rejection, None),
                StatusCode::ACCEPTED,
            ))
        },
    )
}
