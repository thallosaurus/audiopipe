#!/bin/sh
DATE=$(date +"%Y-%m-%dT%H:%M:%S")
mkdir -p logs
RUST_BACKTRACE=1 cargo run --bin receiver -- -d "MacBook Pro Speakers" -c 0 -c 1  2>&1 | tee logs/receiver.$DATE.log