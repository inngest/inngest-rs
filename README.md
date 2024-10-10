<div align="center">
  <a href="https://www.inngest.com">
    <img src="https://user-images.githubusercontent.com/306177/191580717-1f563f4c-31e3-4aa0-848c-5ddc97808a9a.png" width="250" />
  </a>
  <br/>
  <br/>
  <p>
    Write durable functions in Rust via the <a href="https://www.inngest.com">Inngest</a> SDK.<br />
    Read the <a href="https://www.inngest.com/docs?ref=github-inngest-rust-readme">documentation</a> and get started in minutes.
  </p>
  <p>

[![crates.io](https://img.shields.io/crates/v/inngest)](https://crates.io/crates/inngest)
[![discord](https://img.shields.io/discord/842170679536517141?label=discord)](https://www.inngest.com/discord)
[![twitter](https://img.shields.io/twitter/follow/inngest?style=social)](https://twitter.com/inngest)

  </p>
</div>
<hr />

# Inngest Rust SDK

Inngest's SDK adds durable functions to Rust in a few lines of code. Using this SDK, you can write background jobs, and workflows
as step functions without the need to setup queueing infrastructure.

We currently support the following web frameworks:

- [axum](https://github.com/tokio-rs/axum)

If there are other frameworks you like to see, feel free to submit an issue, or add to the [roadmap](https://roadmap.inngest.com/roadmap).

## Getting Started

``` toml
[dependencies]
inngest = "0.1"
```

## Examples

- [axum](./inngest/examples/axum/main.rs)
