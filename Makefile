.PHONY: run build check fmt test clean build-rpi-64 build-rpi-32 build-rpi-cross-64 build-rpi-cross-32

run:
	cargo run --release

build:
	cargo build --release

check:
	cargo check

test:
	cargo test

fmt:
	cargo fmt

# Raspberry Pi 3B targets (64-bit OS default / 32-bit legacy)
build-rpi-64:
	rustup target add aarch64-unknown-linux-gnu
	cargo build --release --target aarch64-unknown-linux-gnu

build-rpi-32:
	rustup target add armv7-unknown-linux-gnueabihf
	cargo build --release --target armv7-unknown-linux-gnueabihf

# Docker-based cross-compilation targets (requires 'cargo install cross')
build-rpi-cross-64:
	rustup target add aarch64-unknown-linux-gnu
	cross build --release --target aarch64-unknown-linux-gnu

build-rpi-cross-32:
	rustup target add armv7-unknown-linux-gnueabihf
	cross build --release --target armv7-unknown-linux-gnueabihf

clean:
	cargo clean
