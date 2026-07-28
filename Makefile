.PHONY: build install web test run

build: web
	cargo build --release

install: web
	cargo install --path . --force

web:
	npm --prefix web ci
	npm --prefix web run build

test:
	npm --prefix web test
	npm --prefix web run build
	cargo fmt --all -- --check
	cargo test
	cargo clippy --all-targets -- -D warnings

run: web
	cargo run -- $(PROJECT)
