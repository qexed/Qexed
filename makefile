plugin-runtime-test:
	cargo build --target=wasm32-unknown-unknown --package hello_world
	cargo run --package qexed