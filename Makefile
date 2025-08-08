PHONY: clean macos_cli

clean:
	@cargo clean
	@rm -rf dump
	@rm -rf logs

macos_cli:
	@cargo build --release --target=aarch64-apple-darwin --bin cli_app
	@cargo build --release --target=aarch64-apple-darwin --bin receiver_pooled
	@cargo build --release --target=aarch64-apple-darwin --bin sender_pooled

raspi_cli:
	@cargo build --release --target=aarch64-unknown-linux-gnu --bin cli_app
	@cargo build --release --target=aarch64-unknown-linux-gnu --bin receiver_pooled
	@cargo build --release --target=aarch64-unknown-linux-gnu --bin sender_pooled

windows_cli:
	@cargo build --release --target=x86_64-pc-windows-msvc --bin cli_app
	@cargo build --release --target=x86_64-pc-windows-msvc --bin receiver_pooled
	@cargo build --release --target=x86_64-pc-windows-msvc --bin sender_pooled