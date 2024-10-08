FROM rust:1.79-bookworm AS build

RUN apt-get update && apt-get install -y adduser ca-certificates tzdata curl dnsutils && update-ca-certificates

WORKDIR /app
COPY . .

RUN cargo build --example axum --release

FROM ubuntu:24.04 AS runner
RUN apt update && apt install -y adduser wget ca-certificates tzdata curl dnsutils && update-ca-certificates

COPY --from=build /app/target/release/examples/axum /bin/main

USER ubuntu
CMD ["main"]
