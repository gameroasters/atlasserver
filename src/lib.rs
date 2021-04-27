#![forbid(unsafe_code)]
#![deny(unused_must_use)]
#![deny(clippy::all)]
#![deny(clippy::pedantic)]
#![deny(clippy::unwrap_used)]
#![deny(clippy::panic)]
#![deny(clippy::perf)]
#![deny(clippy::nursery)]
#![deny(clippy::match_like_matches_macro)]
#![deny(clippy::needless_update)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::upper_case_acronyms)]

pub mod dynamo_util;
pub mod error;
pub mod pbwarp;
pub mod rejection;
pub mod schema;
pub mod userlogin;

use async_trait::async_trait;
use frunk::hlist::HList;
pub use frunk::{hlist, Hlist};
use std::{net::SocketAddr, sync::Arc};
use tracing::Span;
use warp::{
	filters::BoxedFilter,
	reply::Reply,
	trace::{Info, Trace},
	Filter,
};

pub struct Module<S>
where
	S: CustomServer + Sized,
{
	pub name: &'static str,
	#[allow(clippy::type_complexity)]
	pub call: fn(server: Arc<S>) -> BoxedFilter<(Box<dyn Reply>,)>,
}

pub trait CustomServer: Send + Sync + 'static + Sized {
	/// Any types which take a lifetime parameter must have `'static` lifetime, and can be constructed in the table definition to satisfy the lifetime.
	type Resources: HList;

	const MODULES: &'static [Module<Self>];

	#[must_use]
	fn get_module(module_name: &str) -> Option<&Module<Self>> {
		Self::MODULES
			.iter()
			.find(|module| module.name == module_name)
	}

	fn get_resources(&self) -> &Self::Resources;
}
#[async_trait]
pub trait CustomModule: Send + Sync + Sized {
	type Resources: HList;
	fn create_filter<S: ModuleResources<Self>>(
		server: Arc<S>,
	) -> BoxedFilter<(Box<dyn Reply>,)>;
}

pub trait ModuleResources<T: CustomModule>: CustomServer {
	fn get_server_resources(&self) -> <T as CustomModule>::Resources;
}

#[must_use]
pub fn trace_request() -> Trace<impl Fn(Info) -> Span + Clone> {
	// use tracing::field::Empty;
	warp::trace::trace(|info: Info| {
		let span = tracing::info_span!(
			"http",
			path = %info.path(),
		);

		// tracing::trace!(parent: &span, "received request");

		span
	})
}

pub async fn initialize_server<S: CustomServer>(
	server: Arc<S>,
	addr: impl Into<SocketAddr> + Send,
) {
	let cors = warp::cors()
		.allow_any_origin()
		.allow_methods(vec!["GET", "POST"]);

	let mut filters = S::MODULES
		.iter()
		.map(|module| (module.call)(server.clone()));

	if let Some(first) = filters.next() {
		let routes = filters.fold(first, |route, next| {
			route
				.or(next)
				.map(|r| -> Box<dyn Reply> { Box::new(r) })
				.boxed()
		});

		let log = warp::log::custom(move |info| {
			tracing::info!(
				target: "http",
				path = %info.path(),
				method = %info.method(),
				elapsed = %info.elapsed().as_micros(),
				status = %info.status(),
				agent = %info.user_agent().unwrap_or_default()
			);
		});

		let routes = routes
			.with(log) // log filter
			.with(trace_request()) //tracing filter
			.with(cors)
			// TODO: make this modular
			.recover(rejection::handle_rejection);

		warp::serve(routes).run(addr).await
	}
}
