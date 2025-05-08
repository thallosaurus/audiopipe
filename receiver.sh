#!/bin/sh
RUST_BACKTRACE=1 cargo run --bin receiver -- -d "MacBook Pro Speakers" -c 0 -c 1 | tee receiver.log