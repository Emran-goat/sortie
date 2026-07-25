.PHONY: build test clippy clean release publish

build:
	cargo build

release:
	cargo build --release

test:
	cargo test

clippy:
	cargo clippy -- -D warnings

clean:
	cargo clean

publish:
	cargo publish
