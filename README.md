![Build](https://github.com/gameroasters/atlas/workflows/CI/badge.svg)

# atlas

`atlasserver` is a rust library for the purpose of composing REST APIs out of re-usable and extensible modules, specifically with supporting the networking needs of online gaming services in mind.

## How it works

Structs which implement the `CustomModule` trait are joined by an object which implements the `CustomServer` trait, which dispatches the warp filters defined within the modules. `CustomModule`s can work on data through the use of "resources", which are arbitrary types that are stored in an [HList](https://docs.rs/hlist/0.1.2/hlist/).

See the examples in the repo for more details.

## Features

* modular/extendable
* supports JSON/Protobuf payloads
* data storage for dynamodb (can be exchanged)

Endpoints:

* User Registration
* User Login (+ session validation)
