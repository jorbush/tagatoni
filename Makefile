.PHONY: run build check fmt test clean build-rpi-64 build-rpi-32 build-rpi-cross-64 build-rpi-cross-32 service-install service-start service-stop service-restart service-status service-logs service-logs-sys

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
	CROSS_CONTAINER_OPTS="--platform linux/amd64" cross build --release --target aarch64-unknown-linux-gnu

build-rpi-cross-32:
	rustup target add armv7-unknown-linux-gnueabihf
	CROSS_CONTAINER_OPTS="--platform linux/amd64" cross build --release --target armv7-unknown-linux-gnueabihf

# systemd service targets (run these on the Raspberry Pi)
service-install:
	@echo "Installing systemd service file..."
	sudo cp tagatoni.service /etc/systemd/system/tagatoni.service
	sudo systemctl daemon-reload
	sudo systemctl enable tagatoni.service
	@echo "Service installed and enabled. Run 'make service-start' to start it."

service-start:
	sudo systemctl start tagatoni.service

service-stop:
	sudo systemctl stop tagatoni.service

service-restart:
	sudo systemctl restart tagatoni.service

service-status:
	sudo systemctl status tagatoni.service

service-logs:
	tail -f /mnt/drive4/tagatoni/logs/agent.log

service-logs-sys:
	journalctl -u tagatoni.service -f

clean:
	cargo clean
