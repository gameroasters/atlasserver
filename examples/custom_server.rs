use std::sync::Arc;

use atlasserver::*;

struct MyServer {
	resources: <Self as CustomServer>::Resources,
}

impl CustomServer for MyServer {
	type Resources = Hlist![Arc<userlogin::UserLoginResource>];

	const MODULES: &'static [Module<Self>] = &[Module {
		name: "userlogin",
		call: userlogin::UserLogin::create_filter,
	}];

	fn get_resources(&self) -> &Self::Resources {
		&self.resources
	}
}

impl ModuleResources<userlogin::UserLogin> for MyServer {
	fn get_server_resources(
		&self,
	) -> <userlogin::UserLogin as CustomModule>::Resources {
		let (reshaped, _) = self.get_resources().clone().sculpt();
		reshaped
	}
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
	let session_db =
		Arc::new(userlogin::sessions::InMemorySessionDB::default());
	let user_db = Arc::new(
		userlogin::user::in_memory::InMemoryUserDB::default(),
	);

	let server = Arc::new(MyServer {
		resources: hlist![Arc::new(
			userlogin::UserLoginResource::new(session_db, user_db)
		),],
	});

	atlasserver::initialize_server(server, ([0, 0, 0, 0], 8080))
		.await;

	Ok(())
}
