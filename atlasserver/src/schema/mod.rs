#![allow(clippy::all)]
#![allow(clippy::pedantic)]
#![allow(clippy::unwrap_used)]
#![allow(clippy::nursery)]
mod schema;

pub use ::protobuf::Message;
pub use schema::*;

include!(concat!(env!("OUT_DIR"), "/get_schema.rs"));
