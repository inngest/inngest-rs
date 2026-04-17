.PHONY: dev
dev:
	cargo run -p inngest --example axum

.PHONY: send-events
send-events:
	INNGEST_DEV=http://127.0.0.1:8288 cargo run -p inngest --example send_events

.PHONY: test
test:
	cargo test

.PHONY: test-e2e
test-e2e:
	./scripts/run-e2e-tests.sh

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
	inngest dev -v -u http://127.0.0.1:3000/api/inngest
