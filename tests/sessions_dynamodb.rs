use atlasserver::userlogin::sessions::{
	DynamoSessionDB, Session, SessionDB,
};
use json::{object, JsonValue};
use mockito::mock;
use pretty_assertions::assert_eq;
use rusoto_core::{credential::StaticProvider, HttpClient, Region};
use rusoto_dynamodb::DynamoDbClient;

#[tokio::test]
async fn test_session_not_existent() {
	let (db, _) = create_test_ddb_session().await;

	let nonexist_res = db.get("invalid").await;

	assert!(nonexist_res.is_none());
}

#[tokio::test]
async fn test_session_create() {
	let (db, _) = create_test_ddb_session().await;

	let mock = mock_ddb_request_ok("PutItem", object! {}).expect(1);

	let session =
		db.create(Session::new("test-user-id")).await.unwrap();

	mock.assert();

	assert!(!session.is_empty());
}

#[tokio::test]
async fn test_session_valid() {
	let (db, _) = create_test_ddb_session().await;

	let mock = mock_ddb_request_ok(
		"UpdateItem",
		object! {
			Attributes: {
				id: {S: "session"},
				user_id: {S: "uid"},
				valid: {S: "true"},
				//TODO: unhardcode timestamp
				ttl: {N: "10000000000"},
			}
		},
	)
	.expect(1);

	let valid_res = db.get("sessionid").await;

	mock.assert();

	assert!(valid_res.is_some());
	assert!(valid_res.as_ref().unwrap().valid);
	assert_eq!(valid_res.unwrap().user_id, "uid");
}

#[tokio::test]
async fn test_session_invalid() {
	let (db, _) = create_test_ddb_session().await;

	let mock = mock_ddb_request_ok(
		"UpdateItem",
		object! {
			Attributes: {
				id: {S: "session"},
				user_id: {S: "uid"},
				valid: {S: "false"},
				//TODO: unhardcode timestamp
				ttl: {N: "10000000000"},
			}
		},
	)
	.expect(1);

	let valid_res = db.get("sessionid").await;

	mock.assert();

	assert!(valid_res.is_some());
	assert!(!valid_res.as_ref().unwrap().valid);
}

#[tokio::test]
async fn test_session_invalidate() {
	let (db, _) = create_test_ddb_session().await;

	let mock =
		mock_ddb_request_ok("UpdateItem", object! {}).expect(1);

	let res = db.invalidate("").await;

	mock.assert();

	assert!(res.is_some());
}

async fn create_test_ddb_session() -> (DynamoSessionDB, mockito::Mock)
{
	// enable env logger
	let _ = env_logger::try_init();

	let table_name = "table";
	let data = object! {
		LastEvaluatedTableName: "string",
		TableNames: [table_name]
	};

	// DynamoSessionDB::new will call `ListTables`
	let mock = mock_ddb_request_ok("ListTables", data);
	let db = DynamoDbClient::new_with(
		HttpClient::new().unwrap(),
		StaticProvider::new_minimal(
			"foo".to_string(),
			"bar".to_string(),
		),
		Region::Custom {
			name: "local".into(),
			endpoint: mockito::server_url(),
		},
	);

	let db = DynamoSessionDB::new(table_name, db).await.unwrap();
	(db, mock)
}

#[tokio::test]
async fn test_session_invalidate_fail() {
	let (db, _) = create_test_ddb_session().await;

	let mock =
		mock_ddb_request("UpdateItem", object! {}, 501).expect(1);

	let res = db.invalidate("").await;

	mock.assert();

	assert!(res.is_none());
}

fn mock_ddb_request_ok(
	endpoint: &str,
	res: JsonValue,
) -> mockito::Mock {
	mock_ddb_request(endpoint, res, 200)
}

fn mock_ddb_request(
	endpoint: &str,
	res: JsonValue,
	status: usize,
) -> mockito::Mock {
	mock("POST", "/")
		.with_status(status)
		.with_header(
			"x-amz-target",
			format!("DynamoDB_20120810.{}", endpoint).as_str(),
		)
		.with_body(res.dump())
		.create()
}
