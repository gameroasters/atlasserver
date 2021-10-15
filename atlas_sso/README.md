# atlas_sso

An atlas module used for storing Single Sign On token credentials, and linking it to
an `atlasserver` user id.
## Providers supported
 - Facebook
 - Sign In With Apple

## Example
This module just needs to be included in the `CustomServer` implementation for your server
struct, and the `SsoResource` needs to be included in the `Resources`.

As is the case with all `atlasserver` modules, `ModuleResources` must also be implemented.

`AtlasSso` also depends on `UserLoginResource`, so it should be included in the `CustomServer`
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
use atlas_sso::{AtlasSso, SsoResource, InMemorySsoDB};
use std::sync::Arc;

struct MyServer{
    resources: <Self as CustomServer>::Resources,
}

impl CustomServer for MyServer {
    type Resources = Hlist![Arc<SsoResource>, Arc<UserLoginResource>];

    const MODULES: &'static [Module<Self>] = &[
         Module {
             name: "sso",
             call: AtlasSso::create_filter,
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

impl ModuleResources<AtlasSso> for MyServer {
    fn get_server_resources(&self) -> <AtlasSso as CustomModule>::Resources {
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

#[tokio::main]
async fn main() {
    let user_db = Arc::new(InMemoryUserDB::default());
    let session_db = Arc::new(InMemorySessionDB::default());
    let sso_db = Arc::new(InMemorySsoDB::default());

    let server = MyServer {
        resources: hlist![
            Arc::new(SsoResource::new(sso_db, user_db.clone())),
            Arc::new(UserLoginResource::new(session_db, user_db)),
        ]
    };

    let future = atlasserver::init(Arc::new(server), ([0, 0, 0, 0], 8080));
    future.await;
}
```
