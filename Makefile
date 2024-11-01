.PHONY: dev
dev:
	cargo run -p inngest --example axum

.PHONY: test
test:
	cargo test

.PHONY: lint
lint:
	cargo clippy

.PHONY: fmt
fmt:
	cargo fmt

.PHONY: changelog
changelog:
	git cliff -o CHANGELOG.md

.PHONY: bump-version
bump-version:
	git cliff --bump -o CHANGELOG.md

.PHONY: inngest-dev
inngest-dev:
	inngest-cli dev -v -u http://127.0.0.1:3000/api/inngest
