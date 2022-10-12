#!/bin/bash

SOURCE_DIR="../../.."
cargo run -qr --manifest-path manifest_merger/Cargo.toml -- --source-dir $SOURCE_DIR --manifest-dir $SOURCE_DIR/.repo/manifests -t $(nproc --all) $*
