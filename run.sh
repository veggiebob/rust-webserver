#!/bin/bash
cargo build --release
authbind --deep ./target/release/veggiebob-website "$1"
