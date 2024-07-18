shttp - A simple HTTP server written in Rust
============================================

This repo contains the HTTP server provided as a library
and some application examples to demonstrate how to use it.


Example applications
--------------------

Following the [Cargo Package Layout](https://doc.rust-lang.org/cargo/guide/project-layout.html),
examples reside under the `examples` directory:

- `examples/example_name_a.rs` - Single-source file example
- `examples/example_name_b/`   - Multi-source file example

To execute a specific example, run:

```
cargo run --example <example_name_x>
```

### `app_state`

This example shows how to access/share both a fixed app configuration `AppConfig` and a dynamic
state `AppState` among HTTP requests.


### `manbrowser`

Access your local man pages from the web browser, just include the name of the man page in the URL,
for example:

```
http://localhost:7878/bash
```

When starting the application, an HTML renderer different than the default can be specified. This
shows how to add command line options to the ones provided by the server.

NOTE: Non-default renderers require the corresponding utilities to be installed in the system: 
`roffit`, `pandoc`, `man2html`.

For details run:
```
cargo run --example manbrowser -- --help
```

