
check:
	cargo c
	cargo fmt -- --check
	cargo clean -p atlas
	cargo clippy
	cargo t

check-nightly:
	cargo +nightly c
	cargo +nightly clean -p atlas
	cargo +nightly clippy