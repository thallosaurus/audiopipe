#!/bin/sh
DATE=$(date +"%Y-%m-%dT%H:%M:%S")
mkdir -p logs
RUST_BACKTRACE=full RUST_LOG=debug cargo run --bin audiopipe -- -d "BlackHole 64ch" -t 0,1 sender 10.0.0.41 2>&1 | tee logs/receiver.$DATE.log