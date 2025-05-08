#!/bin/sh
DATE=$(date +"%Y-%m-%dT%H:%M:%S")
mkdir -p logs
RUST_BACKTRACE=full cargo run --bin sender -- -vvv -n 10.0.0.41:42069 -d "BlackHole 64ch" -c 0 -c 1 2>&1 | tee logs/sender.$DATE.log