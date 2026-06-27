# Tagatoni 🏷️

Tagatoni is a robust, always-running autonomous agent in the Jorbites ecosystem designed to audit and enrich posted recipes by adding missing fields (`calories` and `recipeCuisine`) for SEO and user experience.

<p align="center">
  <img src="./docs/assets/tagatoni.png" alt="Tagatoni Logo" width="240" />
</p>

See the [Tagatoni Architecture and System Design](./docs/architecture.md) and the [Raspberry Pi SSD Deployment Guide](./docs/raspberry_setup.md) for detailed overviews of the system.

## Features

- **Paced Processing**: Respects the Gemini API free tier limits by processing one recipe at a time with a configurable delay (default: `60` seconds / 1 RPM).
- **Persistent State**: Uses **Turso (libSQL)** to keep track of audited, skipped, and failed recipes.
- **Fail-Safe & Resilient**:
  - Automatically catches all errors (network, parsing, database issues) without panicking.
  - Implements **exponential backoff** for transient API errors (retries up to 3 times per recipe call).
  - Errored recipes are marked for retry after a configurable cooldown (default: `24` hours).
- **Alert Notifications**: Supports optional **SMTP email notifications** if a recipe fails to audit after exhausting all retry counts.
- **Efficient Querying**: Only processes recipes with missing fields in MongoDB.

## Getting Started

### Prerequisites

- Rust (cargo toolchain)
- A Jorbites MongoDB database
- A Turso database setup
- A Gemini API Key (Interactions API / v1beta)

### Configuration

Copy `.env.example` to `.env` and fill in the values:

```bash
cp .env.example .env
```

### Running the Agent

To run the agent in release mode:

```bash
make run
```

Or build the binary:

```bash
make build
```

### Cross-Compilation for Raspberry Pi 3B

Tagatoni provides build targets in the `Makefile` for the Raspberry Pi 3B (which supports both 64-bit and 32-bit OS targets).

1. **Native cross-compilation** (Requires local installation of `gcc-aarch64-linux-gnu` or `gcc-arm-linux-gnueabihf`):
   ```bash
   # For 64-bit OS (e.g. Raspberry Pi OS 64-bit)
   make build-rpi-64

   # For 32-bit OS (legacy)
   make build-rpi-32
   ```

2. **Docker-based cross-compilation** (Recommended, compiles inside containers using pre-configured toolchains):
   ```bash
   # Install cross tool
   cargo install cross

   # Compile for 64-bit OS
   make build-rpi-cross-64

   # Compile for 32-bit OS
   make build-rpi-cross-32
   ```

The compiled binaries will be located under `target/<target-triple>/release/tagatoni`. You can transfer them to your Pi using `scp`. For step-by-step instructions on setting up mounting points, deploying files, and running it as a background service on your Raspberry Pi SSD, see the [Raspberry Pi SSD Deployment Guide](./docs/raspberry_setup.md).
