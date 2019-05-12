all:
	cargo build

test:
	cargo test

release:
	cargo build --release
	strip target/release/azi
