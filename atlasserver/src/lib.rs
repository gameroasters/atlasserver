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
//TODO: remove once this works with async_trait again
#![allow(clippy::no_effect_underscore_binding)]

pub mod error;
/// Utilities for simplifying the use of protobuf in warp filters and reply types
pub mod pbwarp;
/// Rejection handling for reroutable filters
pub mod rejection;
/// protobuf message schema for the `userlogin` atlasserver module
pub mod schema;
/// atlasserver module provided to give a simple status code 200 server response to
/// check that the server is running
pub mod status;
/// atlasserver module provided for basic user authentication and session handling
pub mod userlogin;

use crate::userlogin::HEADER_SESSION;
use async_trait::async_trait;
use frunk::hlist::HList;
pub use frunk::{hlist, Hlist};
use std::{net::SocketAddr, sync::Arc};
use tracing::Span;
use warp::{
	filters::BoxedFilter,
	hyper::header::CONTENT_TYPE,
	reply::Reply,
	trace::{Info, Trace},
	Filter,
};

/// Modules store a function call which will generate a warp filter. Generally this function
/// will be the `CustomModule`'s `create_filter` function. See the documentation for `CustomServer`
/// and `CustomModule` for more details.
pub struct Module<S>
where
	S: CustomServer + Sized,
{
	/// `name` is essentially just for debugging or testing purposes, and can be used to pull
	/// a specific `Module` from the `CustomServer` using `get_module` instead
	/// of going through the regular initialization process to create a filter. It does not
	/// handle name conflicts, so names should be unique.
	pub name: &'static str,
	#[allow(clippy::type_complexity)]
	pub call: fn(server: Arc<S>) -> BoxedFilter<(Box<dyn Reply>,)>,
}

/// The `CustomServer` trait describes which modules will be used by the server, and which resources
/// those modules will need to access in their respective filters.
///
/// # Examples
/// ## Defining a `CustomServer` type
/// ```rust
/// # use atlasserver::{CustomServer, CustomModule, Module, ModuleResources};
/// # use frunk::{hlist, Hlist};
/// # use warp::{Filter, filters::BoxedFilter};
/// # use std::sync::Arc;
/// #
/// # struct MyModule {};
/// #
/// # impl CustomModule for MyModule {
///     # type Resources = Hlist![];
///     # fn create_filter<S: ModuleResources<Self>>(
///         # server: std::sync::Arc<S>,
///     # ) -> warp::filters::BoxedFilter<(Box<dyn warp::Reply>,)> {
///         # warp::any().map(|| -> Box<dyn warp::Reply> { Box::new(warp::reply()) }).boxed()
///     # }
/// # }
/// #
/// # impl ModuleResources<MyModule> for MyServer {
///     # fn get_server_resources(&self) -> <MyModule as CustomModule>::Resources {
///         # hlist![]
///     # }
/// # }
/// #
/// struct MyServer {
///     resources: <Self as CustomServer>::Resources
/// }
///
/// impl CustomServer for MyServer {
///     type Resources = Hlist![Arc<MyModule>];
///
///     const MODULES: &'static [Module<Self>] = &[
///         Module {
///             name: "my_module",
///             call: MyModule::create_filter
///         }
///     ];
///
///     fn get_resources(&self) -> &Self::Resources {
///         &self.resources
///     }
/// }
/// ```
pub trait CustomServer: Send + Sync + 'static + Sized {
	/// An `HList` containing any type that would be required to act on in some way through the filters.
	/// Any persistent data or database accesses should be handled in the resource types.
	/// Any types which take a lifetime parameter must have `'static` lifetime, and can be constructed
	/// in the table definition to satisfy the lifetime.
	type Resources: HList;

	const MODULES: &'static [Module<Self>];

	/// Can be used to execute the `call` from the `Module` manually, typically just for testing or
	/// debugging.
	#[must_use]
	fn get_module(module_name: &str) -> Option<&Module<Self>> {
		Self::MODULES
			.iter()
			.find(|module| module.name == module_name)
	}

	/// Method used to return the underlying resource data.
	///
	/// # Examples
	/// ```rust
	/// # use frunk::Hlist;
	/// # use atlasserver::{CustomServer, Module};
	/// #
	/// struct MyServer {
	///     resources: <Self as CustomServer>::Resources,
	/// }
	///
	/// impl CustomServer for MyServer {
	///     # type Resources = Hlist![];
	///     # const MODULES: &'static [Module<Self>] = &[];
	///     // ...
	///
	///     fn get_resources(&self) -> &Self::Resources {
	///         &self.resources
	///     }
	/// }
	/// ```
	fn get_resources(&self) -> &Self::Resources;
}

/// Modules are typically empty struct types which implement the `CustomModule` trait.
///
/// # Examples
/// ## Defining a `CustomModule` that has two endpoints and utilizes two resources
/// ```rust
/// # use atlasserver::{CustomModule, ModuleResources};
/// # use frunk::Hlist;
/// # use std::sync::Arc;
/// # use warp::{Reply, Rejection, Filter, filters::BoxedFilter};
/// #
/// # struct MyResource;
/// # struct AnotherResource;
/// #
/// struct MyModule;
///
/// impl CustomModule for MyModule {
///     type Resources = Hlist![Arc<MyResource>, Arc<AnotherResource>];
///
///     fn create_filter<S: ModuleResources<Self>>(
///         server: Arc<S>,
///     ) -> BoxedFilter<(Box<dyn Reply>,)> {
///
///         // Get our resources from the server
///         let (reshaped, _): (Self::Resources, _) = server.get_server_resources().sculpt();
///         let (my_resource, another_resource) = reshaped.into_tuple2();
///
///         let some_path = warp::path!("some" / "path")
///             // Use a resource as an argument in the filter function
///             .and(warp::any().map(move || my_resource.clone()))
///             .and_then(some_path_fn);
///
///         let some_other_path = warp::path!("some" / "other" / "path")
///             .and(warp::any().map(move || another_resource.clone()))
///             .and_then(some_other_path_fn);
///
///         // Combine the filters and box them
///         some_path
///             .or(some_other_path)
///             .map(|reply| -> Box<dyn Reply> { Box::new(reply) })
///             .boxed()
///     }
/// }
///
/// async fn some_path_fn(resource: Arc<MyResource>) -> Result<impl Reply, Rejection> {
///     // Do something with the resource here, ie: database input, etc
///     # Ok(warp::reply())
/// }
///
/// async fn some_other_path_fn(resource: Arc<AnotherResource>) -> Result<impl Reply, Rejection> {
///     // Do something with the resource here, ie: database input, etc
///     # Ok(warp::reply())
/// }
/// ```
#[async_trait]
pub trait CustomModule: Send + Sync + Sized {
	/// The `Resources` associated type describes which resources are used by just this module,
	/// so it should only represent a subset of the `CustomServer`'s resources.
	type Resources: HList;
	/// This function can access the server resources via `server.get_server_resources()`, and should
	/// return a combined warp filter representing every route associated with this module.
	fn create_filter<S: ModuleResources<Self>>(
		server: Arc<S>,
	) -> BoxedFilter<(Box<dyn Reply>,)>;
}

/// The `ModuleResources` trait is required to be implemented to the `CustomServer` for each module that is
/// added. This allows the individual modules to access the server's resources. It would typically always
/// be implemented in the exact same way, so it could possibly be handled with a macro in the future.
///
/// # Examples
/// ## Implementing `ModuleResources` for a struct which implements `CustomServer`
/// ```rust
/// # use frunk::Hlist;
/// # use atlasserver::{CustomServer, CustomModule, Module, ModuleResources};
/// # use warp::Filter;
/// #
/// # struct MyModule;
/// #
/// # impl CustomModule for MyModule {
///     # type Resources = Hlist![];
///     # fn create_filter<S: ModuleResources<Self>>(
///         # server: std::sync::Arc<S>,
///     # ) -> warp::filters::BoxedFilter<(Box<dyn warp::Reply>,)> {
///         # warp::any().map(|| -> Box<dyn warp::Reply> { Box::new(warp::reply()) }).boxed()
///     # }
/// # }
/// #
/// # struct MyServer {
///     # resources: <Self as CustomServer>::Resources,
/// # }
/// #
/// # impl CustomServer for MyServer {
///     # type Resources = Hlist![];
///     # const MODULES: &'static [Module<Self>] = &[
///         # Module {
///             # name: "",
///             # call: MyModule::create_filter
///         # }
///     # ];
/// #
///     # fn get_resources(&self) -> &Self::Resources {
///         # &self.resources
///     # }
/// # }
/// #
/// impl ModuleResources<MyModule> for MyServer {
///     fn get_server_resources(&self) -> <MyModule as CustomModule>::Resources {
///         let (reshaped, _) = self.get_resources().clone().sculpt();
///         reshaped
///     }
/// }
/// ```
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

/// Combines all of the filters from the server's modules and initializes the server at
/// the given address.
///
/// # Examples
/// ## Initalizing a `CustomServer` type
/// ```rust
/// # use frunk::{hlist, Hlist};
/// # use atlasserver::{CustomServer, CustomModule, Module, ModuleResources};
/// # use warp::Filter;
/// # use std::sync::Arc;
/// # use futures::future::{Abortable, AbortHandle};
/// #
/// # struct MyResource;
/// #
/// # impl MyResource {
/// #     pub fn new() -> Self { Self }
/// # }
/// #
/// # struct MyModule;
/// #
/// # impl CustomModule for MyModule {
///     # type Resources = Hlist![];
///     # fn create_filter<S: ModuleResources<Self>>(
///         # server: std::sync::Arc<S>,
///     # ) -> warp::filters::BoxedFilter<(Box<dyn warp::Reply>,)> {
///         # warp::any().map(|| -> Box<dyn warp::Reply> { Box::new(warp::reply()) }).boxed()
///     # }
/// # }
/// #
/// # struct MyServer {
///     # resources: <Self as CustomServer>::Resources,
/// # }
/// #
/// # impl CustomServer for MyServer {
///     # type Resources = Hlist![Arc<MyResource>];
///     # const MODULES: &'static [Module<Self>] = &[
///         # Module {
///             # name: "",
///             # call: MyModule::create_filter
///         # }
///     # ];
/// #
///     # fn get_resources(&self) -> &Self::Resources {
///         # &self.resources
///     # }
/// # }
/// #
/// # impl ModuleResources<MyModule> for MyServer {
///     # fn get_server_resources(&self) -> <MyModule as CustomModule>::Resources {
///         # let (reshaped, _) = self.get_resources().clone().sculpt();
///         # reshaped
///     # }
/// # }
/// #
/// #[tokio::main]
/// async fn main() {
///     let my_server = MyServer {
///         resources: hlist![
///             Arc::new(MyResource::new())
///         ]
///     };
///
///     let future = atlasserver::init(Arc::new(my_server), ([0, 0, 0, 0], 8080));
///     # let (abort_handle, abort_registration) = AbortHandle::new_pair();
///     # let future = Abortable::new(future, abort_registration);
///     # abort_handle.abort();
///     future.await;
/// }
/// ```
pub async fn init<S: CustomServer>(
	server: Arc<S>,
	addr: impl Into<SocketAddr> + Send,
) {
	//TODO: make this configurable
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

		warp::serve(routes).run(addr).await;
	}
}

/// Like `init`, but can handle shuttign down the server gracefully through
/// a receiver.
///
/// # Examples
/// ## Initializing and gracefully shutting down
/// ```rust
/// # use frunk::{hlist, Hlist};
/// # use atlasserver::{CustomServer, CustomModule, Module, ModuleResources};
/// # use warp::Filter;
/// # use std::sync::Arc;
/// #
/// # struct MyResource;
/// #
/// # impl MyResource {
/// #     pub fn new() -> Self { Self }
/// # }
/// #
/// # struct MyModule;
/// #
/// # impl CustomModule for MyModule {
///     # type Resources = Hlist![];
///     # fn create_filter<S: ModuleResources<Self>>(
///         # server: std::sync::Arc<S>,
///     # ) -> warp::filters::BoxedFilter<(Box<dyn warp::Reply>,)> {
///         # warp::any().map(|| -> Box<dyn warp::Reply> { Box::new(warp::reply()) }).boxed()
///     # }
/// # }
/// #
/// # struct MyServer {
///     # resources: <Self as CustomServer>::Resources,
/// # }
/// #
/// # impl CustomServer for MyServer {
///     # type Resources = Hlist![Arc<MyResource>];
///     # const MODULES: &'static [Module<Self>] = &[
///         # Module {
///             # name: "",
///             # call: MyModule::create_filter
///         # }
///     # ];
/// #
///     # fn get_resources(&self) -> &Self::Resources {
///         # &self.resources
///     # }
/// # }
/// #
/// # impl ModuleResources<MyModule> for MyServer {
///     # fn get_server_resources(&self) -> <MyModule as CustomModule>::Resources {
///         # let (reshaped, _) = self.get_resources().clone().sculpt();
///         # reshaped
///     # }
/// # }
/// #
/// #[tokio::main]
/// async fn main() {
///     let my_server = MyServer {
///         resources: hlist![
///             Arc::new(MyResource::new())
///         ]
///     };
///
///     let (sender, receiver) = tokio::sync::oneshot::channel();
///     // shutdown after 1 second
///     tokio::spawn(async move {
///         tokio::time::sleep(std::time::Duration::from_secs(1));
///         sender.send(())
///     });
///
///     atlasserver::init_with_graceful_shutdown(
///         Arc::new(my_server),
///         ([0, 0, 0, 0], 8080),
///         receiver
///     ).await;
/// }
/// ```
pub async fn init_with_graceful_shutdown<S: CustomServer>(
	server: Arc<S>,
	addr: impl Into<SocketAddr> + Send,
	shutdown_receiver: tokio::sync::oneshot::Receiver<()>,
) {
	//TODO: make this configurable
	let cors = warp::cors()
		.allow_any_origin()
		.allow_headers([CONTENT_TYPE.as_str(), HEADER_SESSION])
		.allow_methods(vec![
			"GET", "POST", "PUT", "UPDATE", "DELETE",
		]);

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

		let (addr, server) = warp::serve(routes)
			.bind_with_graceful_shutdown(addr.into(), async {
				shutdown_receiver.await.ok();
			});

		tracing::info!("serverstart: {}", addr);

		server.await;
	}
}
