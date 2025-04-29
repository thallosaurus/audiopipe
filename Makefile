PHONY: clean macos_cli

clean:
	@rm -rf target

macos_cli:
	@cargo build --release --target=aarch64-apple-darwin --bin sender
	@cargo build --release --target=aarch64-apple-darwin --bin sender_tcp
	@cargo build --release --target=aarch64-apple-darwin --bin receiver
	@cargo build --release --target=aarch64-apple-darwin --bin receiver_tcp

raspi_cli:
	@cargo build --release --target=aarch64-unknown-linux-gnu --bin sender
	@cargo build --release --target=aarch64-unknown-linux-gnu --bin sender_tcp
	@cargo build --release --target=aarch64-unknown-linux-gnu --bin receiver
	@cargo build --release --target=aarch64-unknown-linux-gnu --bin receiver_tcp

windows_cli:
	@cargo build --release --target=x86_64-pc-windows-msvc --bin sender
	@cargo build --release --target=x86_64-pc-windows-msvc --bin sender_tcp
	@cargo build --release --target=x86_64-pc-windows-msvc --bin receiver
	@cargo build --release --target=x86_64-pc-windows-msvc --bin receiver_tcp
