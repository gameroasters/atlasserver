# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## Unreleased

## Fixes
- logout of a previous session was not working correctly [d385b98]

## Added
- move simple `/status` endpoint into atlas to simplify healthprobes of cloud loadbalancers

## Changed
- `on_login` and `on_register` can now fail and prevent atlas from performing the login/register