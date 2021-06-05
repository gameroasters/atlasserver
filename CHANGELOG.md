# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## Unreleased

## Changed
- update tokio and change dependency to pin only on `1` since its promised to be a stable API
  
## [0.1.1] - 2021-06-05

## Fixes
- logout of a previous session was not working correctly [d385b98]

## Added
- move simple `/status` endpoint into atlas to simplify healthprobes of cloud loadbalancers

## Changed
- `on_login` and `on_register` can now fail and prevent atlas from performing the login/register