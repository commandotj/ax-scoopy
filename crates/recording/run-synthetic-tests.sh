#!/bin/bash
cargo run --package scoopy-recording --example synthetic-test-runner --features test-utils -- "$@"
