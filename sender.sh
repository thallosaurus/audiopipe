#!/bin/sh
RUST_BACKTRACE=1 cargo run --bin sender -- -n 10.0.0.41:42069 -d "BlackHole 64ch" -c 0 -c 1 | tee sender.log