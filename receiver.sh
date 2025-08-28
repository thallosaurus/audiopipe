#!/bin/sh
DATE=$(date +"%Y-%m-%dT%H:%M:%S")
mkdir -p logs
RUST_BACKTRACE=full RUST_LOG=debug cargo run --bin audiopipe -- -d "MacBook Pro Speakers" -t 0,1 receiver 2>&1 | tee logs/receiver.$DATE.log