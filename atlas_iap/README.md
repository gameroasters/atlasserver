# atlas_iap

An atlas module used for validating purchases made through Unity's In App Purchases plugin.
Validated purchases get stored in a receipts database, and trigger a callback which can be
registered to handle giving the players resource, or marking them as subscribed.

## Stores supported
 - Google Play
 - Apple App Store

## Example
This module just needs to be included in the `CustomServer` implementation for your server
struct, and the `IapResource` needs to be included in the `Resources`.

As is the case with all `atlasserver` modules, `ModuleResources` must also be implemented.

`Iap` also depends on `UserLoginResource`, so it should be included in the `CustomServer`
implementation.

```rust
use atlasserver::{
    CustomServer, CustomModule, Module, ModuleResources,
    hlist, Hlist,
    userlogin::{
        user::in_memory::InMemoryUserDB, sessions::InMemorySessionDB,
        UserLogin, UserLoginResource,
    }
};
use atlas_iap::{
    Iap, IapResource, IapEventHandler, InMemoryReceiptDB, Receipt
};
use async_trait::async_trait;
use std::sync::Arc;

struct MyServer{
    resources: <Self as CustomServer>::Resources,
}

impl CustomServer for MyServer {
    type Resources = Hlist![Arc<IapResource>, Arc<UserLoginResource>];

    const MODULES: &'static [Module<Self>] = &[
         Module {
             name: "iap",
             call: Iap::create_filter,
         },
         Module {
             name: "userlogin",
             call: UserLogin::create_filter,
         }
    ];

    fn get_resources(&self) -> &Self::Resources {
        &self.resources
    }
}

impl ModuleResources<Iap> for MyServer {
    fn get_server_resources(&self) -> <Iap as CustomModule>::Resources {
        let (reshaped, _) = self.get_resources().clone().sculpt();
        reshaped
    }
}

impl ModuleResources<UserLogin> for MyServer {
    fn get_server_resources(&self) -> <UserLogin as CustomModule>::Resources {
        let (reshaped, _) = self.get_resources().clone().sculpt();
        reshaped
    }
}

struct MyEventHandler;

#[async_trait]
impl IapEventHandler for MyEventHandler {
    async fn on_valid_receipt(
        &self,
        receipt: &Receipt
    ) -> atlas_iap::error::Result<()> {
        // Handle valid receipt purchases here
    }
}

#[tokio::main]
async fn main() {
    let user_db = Arc::new(InMemoryUserDB::default());
    let session_db = Arc::new(InMemorySessionDB::default());
    let receipt_db = Arc::new(InMemoryReceiptDB::default());

    let mut iap_resource =
        IapResource::new(
            receipt_db,
            Some(String::from("apple_secret")),
            None
        )
        .unwrap();

    iap_resource.set_event_handler(Arc::new(MyEventHandler));

    let server = MyServer {
        resources: hlist![
            Arc::new(iap_resource),
            Arc::new(UserLoginResource::new(session_db, user_db)),
        ]
    };

    let future = atlasserver::init(Arc::new(server), ([0, 0, 0, 0], 8080));
    future.await;
}
```
