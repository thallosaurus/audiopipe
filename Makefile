PHONY: clean build docs

TARGET ?= aarch64-apple-darwin
BIN_NAME = audiopipe
TARGET_DIR = target/$(TARGET)/release
BIN_PATH = $(TARGET_DIR)/$(BIN_NAME)

clean:
	@cargo clean
	@rm -rf dump logs

build:
	@cargo build --release --target=$(TARGET) --bin $(BIN_NAME)

docs:
	@cargo doc --release

macos_cli: target/aarch64-apple-darwin/release/audiopipe
target/aarch64-apple-darwin/release/audiopipe:
	@$(MAKE) build TARGET=aarch64-apple-darwin

windows_cli: target/x86_64-pc-windows-msvc/release/audiopipe
target/x86_64-pc-windows-msvc/release/audiopipe:
	@$(MAKE) build TARGET=x86_64-pc-windows-msvc

raspi64_cli: target/aarch64-unknown-linux-gnu/release/audiopipe
target/aarch64-unknown-linux-gnu/release/audiopipe:
	@$(MAKE) build TARGET=aarch64-unknown-linux-gnu