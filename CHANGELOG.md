# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## Unreleased

## Added
- `user/validate_session` endpoint which returns `RejectionResponse` on non Ok session. Analogous to behaviour of `session_filter`.
If session is Ok, returns empty `ValidateSessionResponse`

## Changed
- change main server start method from verbose `atlasserver::initialize_server` to `atlasserver::init`
- use regular content-type header instead of custom one `x-content-type`

## [0.1.2] - 2021-07-01

## Changed
- update tokio and change dependency to pin only on `1` since its promised to be a stable API
- removed Cargo.lock since we are just a library
- bump `rusoto` to `0.47`

## [0.1.1] - 2021-06-05

## Fixes
- logout of a previous session was not working correctly [d385b98]

## Added
- move simple `/status` endpoint into atlas to simplify healthprobes of cloud loadbalancers

## Changed
- `on_login` and `on_register` can now fail and prevent atlas from performing the login/register
