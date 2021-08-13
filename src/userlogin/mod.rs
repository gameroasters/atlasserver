pub mod ipdb;
pub mod sessions;
pub mod user;

use crate::{
	error, pbwarp, rejection::SessionFailure, schema, CustomModule,
	ModuleResources,
};
use async_trait::async_trait;
use frunk::Hlist;
use ipdb::IpDB;
use sessions::Session;
use std::{net::SocketAddr, sync::Arc};
use tracing::instrument;
use user::{User, UserDB};
use warp::{
	filters::BoxedFilter, hyper::header::CONTENT_TYPE, Filter,
	Rejection, Reply,
};

//TODO: make configurable from using crate
pub const MIN_CLIENT_VERSION: u32 = 1;

//TODO: this shouldn't be defined here
pub const HEADER_SESSION: &str = "X-GR-Session";

//TODO: use everywhere
pub type UserId = String;

/// session validation responses
pub enum SessionValidationResult {
	/// returns `user_id` belonging to the session
	Ok { user_id: UserId },
	/// session is known but was supercedet
	Invalid,
	/// unknown or timedout session
	Unknown,
}

pub struct UserLogin {}

#[async_trait]
pub trait UserLoginEvents: Send + Sync {
	async fn on_login(
		&self,
		_user: &User,
	) -> Result<(), error::Error>;
	async fn on_register(
		&self,
		_user: &User,
	) -> Result<(), error::Error>;
}

pub struct UserLoginResource {
	sessions: Arc<dyn sessions::SessionDB>,
	users: Arc<dyn UserDB>,
	events: Option<Arc<dyn UserLoginEvents>>,
	ipdb: Option<IpDB>,
}

impl UserLoginResource {
	#[must_use]
	pub fn new(
		sessions: Arc<dyn sessions::SessionDB>,
		users: Arc<dyn UserDB>,
	) -> Self {
		Self {
			sessions,
			users,
			events: None,
			ipdb: None,
		}
	}

	///
	pub fn set_events(&mut self, events: Arc<dyn UserLoginEvents>) {
		self.events = Some(events);
	}

	///
	pub fn set_ip_db(&mut self, ipdb: IpDB) {
		self.ipdb = Some(ipdb);
	}

	pub async fn validate_session(
		&self,
		session: &str,
	) -> SessionValidationResult {
		match self.sessions.get(session).await {
			Some(session) if session.valid => {
				SessionValidationResult::Ok {
					user_id: session.user_id,
				}
			}
			Some(_) => SessionValidationResult::Invalid,
			None => SessionValidationResult::Unknown,
		}
	}

	async fn country_from_ip(
		&self,
		ip: Option<String>,
	) -> Option<String> {
		if let Some(ipdb) = self.ipdb.as_ref() {
			ipdb.lookup(&ip.unwrap_or_default()).await
		} else {
			None
		}
	}

	#[instrument(skip(self))]
	async fn user_login(
		&self,
		login_request: schema::LoginRequest,
		ip: Option<String>,
	) -> error::Result<(schema::LoginResponse, String)> {
		if !is_valid_version(login_request.clientVersion) {
			return Ok((
				schema::LoginResponse {
					isOutdated: true,
					..schema::LoginResponse::default()
				},
				String::new(),
			));
		}

		let user_creds = login_request.user.unwrap();
		let mut user = self.users.get_user(&user_creds.id).await;
		if let Some(mut user) = user.take() {
			if user.secret == user_creds.secret
				&& user.id == user_creds.id
			{
				if let Some(session) = user.session {
					self.sessions.invalidate(&session).await;
				}

				let session_id = self
					.sessions
					.create(Session::new(&user.id))
					.await?;
				user.session = Some(session_id.clone());

				//TODO: add last_login to User, update last login here

				user.country = self.country_from_ip(ip).await;
				user.language =
					string_to_option(login_request.clientLanguage);

				user.version += 1;
				if self.users.save_user(&user).await.is_err() {
					tracing::error!("user save error");
				}

				if let Some(events) = self.events.as_ref() {
					events.on_login(&user).await?;
				}

				tracing::info!("user succesfully logged in");

				return Ok((
					schema::LoginResponse {
						isOutdated: false,
						..schema::LoginResponse::default()
					},
					session_id,
				));
			}
		}

		Err(error::Error::Io(std::io::Error::new(
			std::io::ErrorKind::NotFound,
			"failed to retrieve user",
		)))
	}

	#[instrument(skip(self))]
	async fn user_register(
		&self,
		client_version: u32,
		client_language: String,
		ip: Option<String>,
	) -> error::Result<(schema::RegisterResponse, String)> {
		if !is_valid_version(client_version) {
			return Ok((
				schema::RegisterResponse {
					isOutdated: true,
					..schema::RegisterResponse::default()
				},
				String::new(),
			));
		}

		let country = self.country_from_ip(ip).await;
		let client_language = string_to_option(client_language);

		let mut new_user = User::new(country, client_language);
		let session = self
			.sessions
			.create(Session::new(&new_user.id))
			.await
			.unwrap_or_default();
		new_user.session = Some(session.clone());

		self.users.save_user(&new_user).await?;

		if let Some(events) = self.events.as_ref() {
			events.on_register(&new_user).await?;
		}

		tracing::info!("registered user: {}", &new_user.id);

		Ok((
			schema::RegisterResponse {
				user: Some(schema::UserCredentials {
					id: new_user.id,
					secret: new_user.secret,
					..schema::UserCredentials::default()
				})
				.into(),
				isOutdated: false,
				..schema::RegisterResponse::default()
			},
			session,
		))
	}
}

impl CustomModule for UserLogin {
	type Resources = Hlist![Arc<UserLoginResource>];

	fn create_filter<S: ModuleResources<Self>>(
		server: std::sync::Arc<S>,
	) -> warp::filters::BoxedFilter<(Box<dyn warp::Reply>,)> {
		let userlogin = warp::any().map(move || {
			let (resource, _) =
				server.clone().get_server_resources().pluck();
			resource
		});

		let register_filter = warp::path!("user" / "register")
			.and(warp::post())
			.and(warp::header::optional::<String>("X-Forwarded-For"))
			.and(warp::addr::remote())
			.and(pbwarp::protobuf_body::<schema::RegisterRequest>())
			.and(userlogin.clone())
			.and(warp::header::optional::<String>(
				CONTENT_TYPE.as_str(),
			))
			.and_then(register_filter_fn);

		let login_filter = warp::path!("user" / "login")
			.and(warp::post())
			.and(warp::header::optional::<String>("X-Forwarded-For"))
			.and(warp::addr::remote())
			.and(pbwarp::protobuf_body::<schema::LoginRequest>())
			.and(userlogin.clone())
			.and(warp::header::optional::<String>(
				CONTENT_TYPE.as_str(),
			))
			.and_then(login_filter_fn);

		let validate_session_filter =
			warp::path!("user" / "validate_session")
				.and(warp::post())
				.and(userlogin)
				.and(warp::header::header::<String>(HEADER_SESSION))
				.and_then(validate_session_fn);

		let filters: BoxedFilter<(Box<dyn Reply>,)> = login_filter
			.or(register_filter)
			.or(validate_session_filter)
			.map(move |reply| -> Box<dyn Reply> { Box::new(reply) })
			.boxed();

		filters
	}
}

async fn login_filter_fn(
	forward_header: Option<String>,
	addr: Option<SocketAddr>,
	request: schema::LoginRequest,
	user_login_resource: Arc<UserLoginResource>,
	content_type: Option<String>,
) -> Result<impl warp::Reply, Rejection> {
	let ip = forward_header
		.clone()
		.or_else(|| addr.map(|addr| addr.ip().to_string()));

	match user_login_resource.user_login(request, ip).await {
		Ok((response, session_id)) => {
			let reply =
				pbwarp::protobuf_reply(&response, content_type);

			return Ok(warp::reply::with_header(
				warp::reply::with_header(
					reply,
					"Access-Control-Expose-Headers",
					HEADER_SESSION,
				),
				HEADER_SESSION,
				session_id,
			)
			.into_response());
		}
		Err(err) => tracing::error!("{}", err),
	}
	Ok(warp::reply::with_status(
		String::from("failed to login"),
		warp::hyper::StatusCode::BAD_REQUEST,
	)
	.into_response())
}

async fn register_filter_fn(
	forward_header: Option<String>,
	addr: Option<SocketAddr>,
	register_request: schema::RegisterRequest,
	user_login_resource: Arc<UserLoginResource>,
	content_type: Option<String>,
) -> Result<impl warp::Reply, Rejection> {
	let ip = forward_header
		.clone()
		.or_else(|| addr.map(|addr| addr.ip().to_string()));

	match user_login_resource
		.user_register(
			register_request.clientVersion,
			register_request.clientLanguage,
			ip,
		)
		.await
	{
		Ok((response, session_id)) => {
			let reply =
				pbwarp::protobuf_reply(&response, content_type);

			return Ok(warp::reply::with_header(
				warp::reply::with_header(
					reply,
					"Access-Control-Expose-Headers",
					HEADER_SESSION,
				),
				HEADER_SESSION,
				session_id,
			)
			.into_response());
		}
		Err(err) => tracing::error!("{}", err),
	}

	Ok(warp::reply::with_status(
		String::from("failed to register user"),
		warp::hyper::StatusCode::BAD_REQUEST,
	)
	.into_response())
}

async fn validate_session_fn(
	resource: Arc<UserLoginResource>,
	session: String,
) -> Result<impl warp::Reply, Rejection> {
	handle_session(resource, session)
		.await
		.map(|_| warp::reply())
}

/// Returns filter that checks session status, which returns rejection if session is not Ok.
/// If session is Ok, request passes through normally
///
/// Intended to be used for composing warp filters
pub fn session_filter(
	resource: Arc<UserLoginResource>,
) -> impl Filter<Extract = (String,), Error = Rejection> + Clone {
	warp::any()
		.map(move || resource.clone())
		.and(warp::header::header::<String>(HEADER_SESSION))
		.and_then(handle_session)
}

async fn handle_session(
	resource: Arc<UserLoginResource>,
	session: String,
) -> Result<String, Rejection> {
	match resource.validate_session(&session).await {
		SessionValidationResult::Ok { user_id } => Ok(user_id),
		SessionValidationResult::Invalid => {
			Err(warp::reject::custom(SessionFailure::Invalid))
		}
		SessionValidationResult::Unknown => {
			Err(warp::reject::custom(SessionFailure::SessionNotFound))
		}
	}
}

#[must_use]
pub const fn is_valid_version(client_version: u32) -> bool {
	client_version >= MIN_CLIENT_VERSION
}

fn string_to_option(string: String) -> Option<String> {
	if string.is_empty() {
		None
	} else {
		Some(string)
	}
}

#[cfg(test)]
mod tests {
	use crate::{
		rejection::{self, handle_rejection},
		schema::{self, RegisterResponse},
		userlogin::{
			session_filter,
			sessions::{InMemorySessionDB, Session, SessionDB},
			user::{in_memory::InMemoryUserDB, User, UserDB},
			UserLogin, UserLoginResource, HEADER_SESSION,
		},
		CustomModule, CustomServer, Module, ModuleResources,
	};
	use frunk::{hlist, Hlist};
	use protobuf::Message;
	use std::{collections::HashMap, sync::Arc};
	use tokio::sync::Mutex;
	use uuid::Uuid;
	use warp::{hyper::StatusCode, Filter};

	pub struct InMemoryServer {
		resources: Hlist![Arc<UserLoginResource>],
	}

	impl CustomServer for InMemoryServer {
		type Resources = Hlist![Arc<UserLoginResource>];

		const MODULES: &'static [Module<Self>] = &[Module {
			name: "userlogin",
			call: UserLogin::create_filter,
		}];

		fn get_resources(&self) -> &Self::Resources {
			&self.resources
		}
	}

	impl ModuleResources<UserLogin> for InMemoryServer {
		fn get_server_resources(
			&self,
		) -> <UserLogin as CustomModule>::Resources {
			let (resources, _) =
				self.get_resources().clone().sculpt();
			resources
		}
	}

	fn sessions_with_session(
		id: &str,
		session: Session,
	) -> Arc<InMemorySessionDB> {
		let mut hashmap = HashMap::new();
		hashmap.insert(id.to_string(), session);
		Arc::new(InMemorySessionDB {
			db: Arc::new(Mutex::new(hashmap)),
		})
	}

	fn users_with_session(
		session: Option<String>,
	) -> Arc<InMemoryUserDB> {
		let mut hashmap = HashMap::new();
		hashmap.insert(
			"uid".to_string(),
			User {
				id: "uid".to_string(),
				session,
				..User::default()
			},
		);
		Arc::new(InMemoryUserDB {
			db: Arc::new(Mutex::new(hashmap)),
		})
	}

	#[tokio::test]
	async fn test_login_from_other_device() {
		let sessions =
			sessions_with_session("sid", Session::new("uid"));
		let users = Arc::new(InMemoryUserDB::default());

		let secret = Uuid::new_v4().to_string();
		let id = Uuid::new_v4().to_string();
		users
			.save_user(&User {
				id: id.clone(),
				secret: secret.clone(),
				session: Some("sid".to_string()),
				..User::default()
			})
			.await
			.ok();

		let server = Arc::new(InMemoryServer {
			resources: hlist![Arc::new(UserLoginResource::new(
				sessions.clone(),
				users.clone()
			))],
		});

		let (user_login_resource, _) =
			server.get_server_resources().pluck();

		let (_, _) = user_login_resource
			.user_login(
				schema::LoginRequest {
					user: Some(schema::UserCredentials {
						id,
						secret,
						..schema::UserCredentials::default()
					})
					.into(),
					clientVersion: 10000,
					..schema::LoginRequest::default()
				},
				None,
			)
			.await
			.unwrap();

		assert!(!sessions.get("sid").await.unwrap().valid);
	}

	#[tokio::test]
	async fn test_session_notfound() {
		let sessions =
			sessions_with_session("sid", Session::new("uid"));

		let users = users_with_session(Some("sid1".to_string()));

		let server = Arc::new(InMemoryServer {
			resources: hlist![Arc::new(UserLoginResource::new(
				sessions.clone(),
				users.clone()
			))],
		});

		let (user_login_resource, _) =
			server.get_server_resources().pluck();
		let filter = warp::path!("test")
			.and(session_filter(user_login_resource))
			.recover(rejection::handle_rejection);

		let reply = warp::test::request()
			.header(HEADER_SESSION, "sid1")
			.path("/test")
			.reply(&filter)
			.await;

		assert_eq!(reply.status(), StatusCode::ACCEPTED);
		let expect = schema::RejectionResponse {
            sessionFilterRejection:
                schema::RejectionResponse_SessionFilterRejection::SESSION_NOT_FOUND,
            ..schema::RejectionResponse::default()
        };
		assert_eq!(
			schema::RejectionResponse::parse_from_bytes(reply.body())
				.unwrap(),
			expect
		);
	}

	#[tokio::test]
	async fn test_session_invalid() {
		let sessions = sessions_with_session(
			"sid1",
			Session {
				user_id: "uid".to_string(),
				valid: false,
			},
		);

		let users = users_with_session(Some("sid1".to_string()));

		let server = Arc::new(InMemoryServer {
			resources: hlist![Arc::new(UserLoginResource::new(
				sessions.clone(),
				users.clone()
			))],
		});

		let (user_login_resource, _) =
			server.get_server_resources().pluck();
		let filter = warp::path!("test")
			.and(session_filter(user_login_resource))
			.recover(handle_rejection);

		let reply = warp::test::request()
			.header(HEADER_SESSION, "sid1")
			.path("/test")
			.reply(&filter)
			.await;

		assert_eq!(reply.status(), StatusCode::ACCEPTED);
		let expect = schema::RejectionResponse {
            sessionFilterRejection: schema::RejectionResponse_SessionFilterRejection::INVALID,
            ..schema::RejectionResponse::default()
        };
		assert_eq!(
			schema::RejectionResponse::parse_from_bytes(reply.body())
				.unwrap(),
			expect
		);
	}

	#[tokio::test]
	async fn test_session_filter() {
		let sessions =
			sessions_with_session("sid", Session::new("uid"));

		let users = users_with_session(Some("sid".to_string()));

		let server = Arc::new(InMemoryServer {
			resources: hlist![Arc::new(UserLoginResource::new(
				sessions.clone(),
				users.clone()
			))],
		});

		let (user_login_resource, _) =
			server.get_server_resources().pluck();
		let filter = warp::path!("test")
			.and(session_filter(user_login_resource))
			.recover(handle_rejection);

		let reply = warp::test::request()
			.header(HEADER_SESSION, "sid")
			.path("/test")
			.reply(&filter)
			.await;

		assert_eq!(reply.status(), 200);
		assert_eq!(reply.body(), "uid");
	}

	#[tokio::test]
	async fn test_reg_response() {
		let sessions = Arc::new(InMemorySessionDB::default());
		let users = Arc::new(InMemoryUserDB::default());

		let server = Arc::new(InMemoryServer {
			resources: hlist![Arc::new(UserLoginResource::new(
				sessions.clone(),
				users.clone()
			))],
		});

		let filter = UserLogin::create_filter(server);
		let request = schema::RegisterRequest {
			clientVersion: 1000000,
			..schema::RegisterRequest::default()
		};
		let reply = warp::test::request()
			.method("POST")
			.body(request.write_to_bytes().unwrap())
			.path("/user/register")
			.reply(&filter)
			.await;

		assert_eq!(reply.status(), 200);
		assert_ne!(
			reply.headers()[HEADER_SESSION],
			String::default()
		);

		let _request =
			schema::RegisterResponse::parse_from_bytes(reply.body())
				.unwrap();
	}

	#[cfg(feature = "json-proto")]
	#[tokio::test]
	async fn test_json_request() {
		use super::CONTENT_TYPE;

		let sessions = Arc::new(InMemorySessionDB::default());
		let users = Arc::new(InMemoryUserDB::default());

		let server = Arc::new(InMemoryServer {
			resources: hlist![Arc::new(UserLoginResource::new(
				sessions.clone(),
				users.clone()
			))],
		});

		let filter = UserLogin::create_filter(server);
		let reply = warp::test::request()
			.method("POST")
			.body(
				r#"
                {
					"clientLanguage": "en-CA",
                    "clientVersion": 1000000
                }
            "#,
			)
			.header(CONTENT_TYPE, "application/json")
			.path("/user/register")
			.reply(&filter)
			.await;

		assert_eq!(reply.status(), 200);
		assert_ne!(
			reply.headers()[HEADER_SESSION],
			String::default()
		);

		let _request: RegisterResponse =
			serde_json::from_slice(&reply.body()).unwrap();
	}

	#[tokio::test]
	async fn test_user_reg() {
		let sessions = Arc::new(InMemorySessionDB::default());
		let users = Arc::new(InMemoryUserDB::default());

		let server = Arc::new(InMemoryServer {
			resources: hlist![Arc::new(UserLoginResource::new(
				sessions.clone(),
				users.clone()
			))],
		});

		let (user_login_resource, _) =
			server.get_server_resources().pluck();

		let (response, session) = user_login_resource
			.user_register(1000000, "en-CA".to_string(), None)
			.await
			.unwrap();

		assert_ne!(session, String::default());
		let response: schema::RegisterResponse = response;
		let user = response.user.unwrap();

		let db_user = users.clone().get_user(&user.id).await.unwrap();
		assert_eq!(db_user.id, user.id);
		assert_eq!(db_user.language, Some("en-CA".to_string()));
		assert_eq!(db_user.session, Some(session.clone()));
		assert_eq!(db_user.secret, user.secret);
		assert_eq!(
			sessions.clone().get(&session).await.unwrap().user_id,
			user.id
		);
	}

	#[tokio::test]
	async fn test_user_login() {
		let sessions = Arc::new(InMemorySessionDB::default());
		let users = Arc::new(InMemoryUserDB::default());

		let secret = Uuid::new_v4().to_string();
		let id = Uuid::new_v4().to_string();
		users
			.save_user(&User {
				id: id.clone(),
				secret: secret.clone(),
				..User::default()
			})
			.await
			.ok();

		let server = Arc::new(InMemoryServer {
			resources: hlist![Arc::new(UserLoginResource::new(
				sessions.clone(),
				users.clone()
			))],
		});

		let (user_login_resource, _) =
			server.get_server_resources().pluck();

		let (response, session) = user_login_resource
			.user_login(
				schema::LoginRequest {
					user: Some(schema::UserCredentials {
						id,
						secret,
						..schema::UserCredentials::default()
					})
					.into(),
					clientVersion: 10000,
					clientLanguage: "en-CA".to_string(),
					..schema::LoginRequest::default()
				},
				None,
			)
			.await
			.unwrap();

		assert_ne!(session, String::default());
		let response: schema::LoginResponse = response;
		assert_eq!(response.isOutdated, false);

		let db_session = sessions.get(&session).await.unwrap();
		let db_user =
			users.get_user(&db_session.user_id).await.unwrap();
		assert_eq!(db_user.language, Some("en-CA".to_string()));
		assert_eq!(db_user.session, Some(session.clone()));
	}

	#[tokio::test]
	async fn test_empty_bytes() {
		let sessions = Arc::new(InMemorySessionDB::default());
		let users = Arc::new(InMemoryUserDB::default());

		let server = Arc::new(InMemoryServer {
			resources: hlist![Arc::new(UserLoginResource::new(
				sessions.clone(),
				users.clone()
			))],
		});

		let filter = UserLogin::create_filter(server);

		let reply = warp::test::request()
			.method("POST")
			.body(&[])
			.path("/user/register")
			.reply(&filter)
			.await;

		assert_eq!(reply.status(), 200);

		let response =
			schema::RegisterResponse::parse_from_bytes(reply.body())
				.unwrap();
		assert!(response.isOutdated);
	}
}
