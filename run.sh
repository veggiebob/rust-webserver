#!/bin/bash
if [ $# -ne 2 ]
then
  echo "Needs at least 2 arguments: <website file location> <addr:port>"
else
  cargo build --release
  authbind --deep ./target/release/veggiebob-website "$1" "$2"
fi