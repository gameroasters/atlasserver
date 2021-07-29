
check:
	cargo c
	cargo fmt -- --check
	cargo clean -p atlasserver
	cargo clippy
	cargo t
	cargo c --example custom_server
	cargo c --example graceful_shutdown

check-nightly:
	cargo +nightly c
	cargo +nightly clean -p atlasserver
	cargo +nightly clippy