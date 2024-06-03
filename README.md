# Kinesis

Kinesis is a asset-oriented programming model powered by the
[Move programming language](https://move-language.github.io/move/introduction.html). Kinesis is a project under active
development by the [IOTA Foundation](https://iota.org).

## Setup

- [Install Rust][install-rust]

### Conventions

The Rust language conventions used in this repository can be found in [Rust Conventions](./RUST_CONVENTIONS.md).

### Formatting

In order to use the unstable features specified in rustfmt.toml, you must have the correct nightly toolchain component
installed.

```sh
rustup toolchain install nightly --component rustfmt --allow-downgrade
```

This can be used regardless of the default toolchain to format the code using the following command.

```sh
cargo +nightly fmt
```

#### IDE Configuration

For convenience, it is recommended that developers configure their IDEs to automatically format files on save.

#### VS Code

`settings.json`

```json
{
  "[rust]": {
    "editor.formatOnSave": true,
  },
  "rust-analyzer.rustfmt.extraArgs": [
    "+nightly"
  ]
}
```

[install-rust]: https://www.rust-lang.org/tools/install
