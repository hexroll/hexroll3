#!/bin/bash
 SCCACHE_C_CUSTOM_CACHE_BUSTER=99 RUST_LOG=info RUST_BACKTRACE=full cargo run --release --features=dev,soft_shadows &
 SCCACHE_C_CUSTOM_CACHE_BUSTER=99 RUST_LOG=info RUST_BACKTRACE=full cargo run --release --features=dev,soft_shadows &

read -n 1 -s -r -p "Press any key to continue"

pkill -P $$

