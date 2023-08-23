.PHONY: dev
dev:
	cargo run -p inngest --example axum

.PHONY: test
test:
	cargo test

.PHONY: lint
lint:
	cargo clippy

.PHONY: inngest-dev
inngest-dev:
	inngest-cli dev -u http://127.0.0.1:3000/api/inngest
