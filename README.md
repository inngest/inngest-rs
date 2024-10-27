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

```rs
use axum::{
  routing::{get, put},
  Router
};

#[tokio::main]
async fn main() {
    let client = Inngest::new("rust-app");
    let mut inngest_handler = Handler::new(&client);
    inngest_handler.register_fns(vec![
         hello_fn(&client)
     ]);

    let inngest_state = Arc::new(inngest_handler);

    let app = Router::new()
        .route("/", get(|| async { "OK!\n" }))
        .route(
            "/api/inngest",
            put(serve::axum::register).post(serve::axum::invoke),
        )
        .with_state(inngest_state);

    let addr = "[::]:3000".parse::<std::net::SocketAddr>().unwrap();

    // run it with hyper on localhost:3000
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(untagged)]
enum Data {
    Hello { msg: String },
}

fn hello_fn(client: &Inngest) -> ServableFn<Data, Error> {
    client.create_function(
        FunctionOpts::new("hello-func").name("Hello func"),
        Trigger::event("test/hello"),
        |input: Input<Data>, step: StepTool| async move {
            let step_res = into_dev_result!(
                step.run("fallible-step-function", || async move {
                    // if even, fail
                    if input.ctx.attempt % 2 == 0 {
                        return Err(UserLandError::General(format!(
                            "Attempt {}",
                            input.ctx.attempt
                        )));
                    }

                    Ok(json!({ "returned from within step.run": true }))
                }).await
            )?;

            step.sleep("sleep-test", Duration::from_secs(3))?;

            let evt: Option<Event<Value>> = step.wait_for_event(
                "wait",
                WaitForEventOpts {
                    event: "test/wait".to_string(),
                    timeout: Duration::from_secs(60),
                    if_exp: None,
                },
            )?;

            Ok(json!({ "hello": true, "evt": evt, "step": step_res }))
        },
    )
}
```
