# BHCLI Makefile - Modern Cargo-based build system
# Works with both rustup and system-installed Rust (Homebrew, apt, etc.)

.PHONY: help build release install clean test check fmt clippy \
        build-linux build-macos build-windows build-all \
        setup-targets install-rust-targets

# Default target
help:
	@echo "BHCLI Build System"
	@echo "=================="
	@echo ""
	@echo "Development:"
	@echo "  make build          - Debug build for current platform"
	@echo "  make release        - Optimized release build"
	@echo "  make install        - Install to ~/.cargo/bin"
	@echo "  make test           - Run all tests"
	@echo "  make check          - Check code without building"
	@echo "  make fmt            - Format code with rustfmt"
	@echo "  make clippy         - Run clippy linter"
	@echo "  make clean          - Clean build artifacts"
	@echo ""
	@echo "Building:"
	@echo "  make build-local         - Optimized build for current platform (with audio)"
	@echo "  make build-linux-musl    - Static Linux binary (no audio, portable)"
	@echo "  make build-windows       - Windows binary (no audio, portable)"
	@echo "  make build-all           - Build for all available platforms"
	@echo ""
	@echo "Features:"
	@echo "  make build-no-audio      - Build current platform without audio"
	@echo "  make build-linux-audio   - Linux build with audio (native only)"
	@echo ""
	@echo "Setup (if using rustup):"
	@echo "  make setup-targets       - Install cross-compile targets"
	@echo "  make show-targets        - Show installed targets"
	@echo ""

# Detect if rustup is available
RUSTUP_EXISTS := $(shell command -v rustup 2> /dev/null)

# Development builds
build:
	cargo build

release:
	cargo build --release

# Install to system
install:
	cargo install --path .

# Testing and verification
test:
	cargo test --all-features

check:
	cargo check --all-features

fmt:
	cargo fmt --all

clippy:
	cargo clippy --all-features -- -D warnings

clean:
	cargo clean
	rm -rf dist/

# Feature variants
build-no-audio:
	cargo build --release --no-default-features

# Setup cross-compilation targets (only works with rustup)
setup-targets:
ifdef RUSTUP_EXISTS
	@echo "Installing Rust cross-compilation targets..."
	@echo ""
	rustup target add x86_64-unknown-linux-musl || true
	rustup target add x86_64-pc-windows-gnu || true
	@echo ""
	@echo "✓ Targets installed"
	@echo ""
	@echo "Note: For full cross-compilation you may also need:"
	@echo "  - musl-tools (Linux): apt install musl-tools"
	@echo "  - mingw-w64 (Windows): apt install mingw-w64"
	@echo ""
else
	@echo "Rustup not found. You have a system Rust installation."
	@echo ""
	@echo "Cross-compilation targets are typically not needed with system Rust."
	@echo "Just use 'make release' to build for your current platform."
	@echo ""
	@echo "If you need cross-compilation, consider using rustup:"
	@echo "  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
	@echo ""
endif

install-rust-targets: setup-targets

# Build for current platform (works with any Rust installation)
build-local:
	@echo "Building optimized binary for current platform..."
	@echo "(With audio support)"
	@mkdir -p dist
	cargo build --release
	@cp target/release/bhcli dist/bhcli-local 2>/dev/null || \
		cp target/release/bhcli.exe dist/bhcli-local.exe 2>/dev/null || \
		echo "Warning: Could not copy binary to dist/"
	@echo "✓ Binary created in dist/ (with audio support)"
	@ls -lh dist/bhcli-* 2>/dev/null || ls -lh target/release/bhcli* 2>/dev/null

# Check if a specific target is available
check-target-musl:
ifdef RUSTUP_EXISTS
	@rustup target list | grep -q "x86_64-unknown-linux-musl (installed)" || \
		(echo "Target not installed. Run: make setup-targets" && exit 1)
else
	@echo "Attempting build without rustup target checking..."
endif

check-target-windows:
ifdef RUSTUP_EXISTS
	@rustup target list | grep -q "x86_64-pc-windows-gnu (installed)" || \
		(echo "Target not installed. Run: make setup-targets" && exit 1)
else
	@echo "Attempting build without rustup target checking..."
endif

# Cross-platform builds
build-linux-musl:
	@echo "Building static Linux binary (x86_64-musl)..."
	@echo "(Building without audio for portability)"
	@mkdir -p dist
	@if cargo build --release --target x86_64-unknown-linux-musl --no-default-features 2>/dev/null; then \
		cp target/x86_64-unknown-linux-musl/release/bhcli dist/bhcli-linux-x86_64 && \
		echo "✓ Linux binary created: dist/bhcli-linux-x86_64" && \
		echo "  Note: Audio disabled for maximum compatibility"; \
	else \
		echo ""; \
		echo "Cross-compilation to x86_64-unknown-linux-musl failed."; \
		echo ""; \
		if [ -n "$(RUSTUP_EXISTS)" ]; then \
			echo "You may need:"; \
			echo "  1. rustup target add x86_64-unknown-linux-musl"; \
			echo "  2. apt install musl-tools (on Linux)"; \
		else \
			echo "Your system Rust may not support this target."; \
			echo "Consider using rustup for cross-compilation."; \
		fi; \
		echo ""; \
		exit 1; \
	fi

# Linux build with audio (for native compilation)
build-linux-audio:
	@echo "Building Linux binary with audio support..."
	@mkdir -p dist
	cargo build --release
	@cp target/release/bhcli dist/bhcli-linux-audio
	@echo "✓ Linux binary with audio: dist/bhcli-linux-audio"

build-macos:
	@echo "Building macOS binary..."
	@mkdir -p dist
	@if cargo build --release 2>/dev/null; then \
		cp target/release/bhcli dist/bhcli-macos && \
		echo "✓ macOS binary created: dist/bhcli-macos"; \
	else \
		echo "Build failed"; \
		exit 1; \
	fi

build-windows:
	@echo "Building Windows binary..."
	@echo "(Building without audio for portability)"
	@mkdir -p dist
	@if cargo build --release --target x86_64-pc-windows-gnu --no-default-features 2>/dev/null; then \
		cp target/x86_64-pc-windows-gnu/release/bhcli.exe dist/bhcli-windows-x86_64.exe && \
		echo "✓ Windows binary created: dist/bhcli-windows-x86_64.exe" && \
		echo "  Note: Audio disabled for maximum compatibility"; \
	else \
		echo ""; \
		echo "Cross-compilation to Windows failed."; \
		echo ""; \
		if [ -n "$(RUSTUP_EXISTS)" ]; then \
			echo "You may need:"; \
			echo "  1. rustup target add x86_64-pc-windows-gnu"; \
			echo "  2. apt install mingw-w64 (on Linux)"; \
		else \
			echo "Your system Rust may not support this target."; \
			echo "Consider using rustup for cross-compilation."; \
		fi; \
		echo ""; \
		exit 1; \
	fi

# Build for all supported platforms
build-all:
	@echo "Building for all available platforms..."
	@echo ""
	@mkdir -p dist
	@BUILD_SUCCESS=0; \
	echo "Attempting Linux static build..."; \
	if $(MAKE) build-linux-musl 2>/dev/null; then \
		BUILD_SUCCESS=1; \
	else \
		echo "⚠ Linux build skipped (target not available)"; \
	fi; \
	echo ""; \
	echo "Attempting Windows build..."; \
	if $(MAKE) build-windows 2>/dev/null; then \
		BUILD_SUCCESS=1; \
	else \
		echo "⚠ Windows build skipped (target not available)"; \
	fi; \
	echo ""; \
	echo "Building for current platform..."; \
	$(MAKE) build-local; \
	BUILD_SUCCESS=1; \
	echo ""; \
	echo "========================================="; \
	echo "Build complete!"; \
	echo "========================================="; \
	ls -lh dist/ 2>/dev/null || echo "Binaries in target/release/"

# Distribution packages
dist:
	@echo "Creating distribution packages..."
	@mkdir -p dist
	@$(MAKE) build-all
	@if [ -f dist/bhcli-linux-x86_64 ]; then \
		cd dist && tar -czf bhcli-linux-x86_64.tar.gz bhcli-linux-x86_64 && \
		echo "✓ Created bhcli-linux-x86_64.tar.gz"; \
	fi
	@if [ -f dist/bhcli-windows-x86_64.exe ]; then \
		cd dist && zip -q bhcli-windows-x86_64.zip bhcli-windows-x86_64.exe && \
		echo "✓ Created bhcli-windows-x86_64.zip"; \
	fi
	@if [ -f dist/bhcli-local ]; then \
		cd dist && tar -czf bhcli-local.tar.gz bhcli-local && \
		echo "✓ Created bhcli-local.tar.gz"; \
	fi
	@echo ""
	@echo "Distribution packages:"
	@ls -lh dist/*.tar.gz dist/*.zip 2>/dev/null || echo "No packages created"

# Verify release build
verify-release:
	@echo "Verifying release build..."
	@cargo build --release
	@echo "✓ Release build successful"
	@echo ""
	@echo "Binary location:"
	@ls -lh target/release/bhcli* 2>/dev/null || echo "Check target/release/"

# Development helpers
watch:
	@command -v cargo-watch >/dev/null 2>&1 || \
		(echo "cargo-watch not installed. Install with: cargo install cargo-watch" && exit 1)
	cargo watch -x 'check --all-features' -x 'test --all-features'

run:
	cargo run --release

# Cargo commands wrapped for convenience
update:
	cargo update

doc:
	cargo doc --no-deps --open

# Show installed targets (if using rustup)
show-targets:
ifdef RUSTUP_EXISTS
	@echo "Installed Rust targets:"
	@rustup target list | grep installed
else
	@echo "Rustup not found. You have a system Rust installation."
	@echo ""
	@echo "Your Rust compiler:"
	@rustc --version
	@echo ""
	@echo "Your Cargo version:"
	@cargo --version
	@echo ""
	@echo "System Rust typically supports your native platform only."
	@echo "For cross-compilation, consider using rustup."
endif

# Show Rust installation info
rust-info:
	@echo "Rust Installation Information"
	@echo "=============================="
	@echo ""
	@echo "Rustc version:"
	@rustc --version
	@echo ""
	@echo "Cargo version:"
	@cargo --version
	@echo ""
ifdef RUSTUP_EXISTS
	@echo "Rustup version:"
	@rustup --version
	@echo ""
	@echo "Active toolchain:"
	@rustup show active-toolchain
	@echo ""
	@echo "Installed targets:"
	@rustup target list | grep installed
else
	@echo "Installation type: System Rust (Homebrew, apt, or other)"
	@echo ""
	@echo "Note: System Rust installations typically support your"
	@echo "      native platform only. For cross-compilation, consider"
	@echo "      using rustup."
endif
	@echo ""
	@echo "To check if cross-compilation works, try:"
	@echo "  make build-all"
