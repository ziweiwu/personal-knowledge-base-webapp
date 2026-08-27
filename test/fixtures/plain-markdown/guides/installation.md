# Installation

[Up to the guides index](./index.md) · [up to the handbook root](../README.md) ·
[up to contributing](../contributing.md)

## Requirements

- Rust 1.80 or newer
- A folder of documents

## Steps

1. Build the server.
2. Point it at a root.
3. Open the printed URL.

```sh
cargo build --release
./target/release/kbview --root ./test/fixtures/plain-markdown --port 4321
```

## Next

Read [configuration](./configuration.md), then the
[CLI reference](../reference/cli.md#flags).

![Screenshot of the reader](../assets/screenshot.png)
