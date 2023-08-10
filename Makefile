.PHONY: dev
dev:
	cargo run --bin dev

.PHONY: test
test:
	cargo test

.PHONY: lint
lint:
	cargo clippy
