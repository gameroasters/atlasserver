use frunk::Hlist;
use warp::{filters::BoxedFilter, Filter, Reply};

use crate::{CustomModule, ModuleResources};

pub struct Status {}

impl CustomModule for Status {
	type Resources = Hlist!();

	fn create_filter<S: ModuleResources<Self>>(
		_: std::sync::Arc<S>,
	) -> warp::filters::BoxedFilter<(Box<dyn warp::Reply>,)> {
		let filter: BoxedFilter<(Box<dyn Reply>,)> =
			warp::path!("status")
				.map(warp::reply::reply)
				.map(|reply| -> Box<dyn Reply> { Box::new(reply) })
				.boxed();
		filter
	}
}
