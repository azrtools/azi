all:
	cargo build

release:
	cargo test
	cargo build --release
	strip target/release/azi
