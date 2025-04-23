PHONY: macos raspi windows

clean:
	@rm -rf target

macos:
	@cargo build --release --target=aarch64-apple-darwin --bin sender
	@cargo build --release --target=aarch64-apple-darwin --bin receiver

raspi:
	@cargo build --release --target=aarch64-unknown-linux-gnu --bin sender
	@cargo build --release --target=aarch64-unknown-linux-gnu --bin receiver

windows:
	@cargo build --release --target=x86_64-pc-windows-msvc --bin sender
	@cargo build --release --target=x86_64-pc-windows-msvc --bin receiver
