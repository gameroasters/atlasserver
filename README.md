![Build](https://github.com/gameroasters/atlas/workflows/CI/badge.svg)

# atlas

atlas is a rust library for the purpose of composing REST APIs out of re-usable and extensible modules, specifically with supporting the networking needs of online gaming services in mind.

## How it works

Structs which implement the `CustomModule` trait are joined by an object which implements the `CustomServer` trait, which dispatches the warp filters defined within the modules. `CustomModule`s can work on data through the use of "resources", which are arbitrary types that are stored in an `Hlist`.

See the examples in the repo for more details.