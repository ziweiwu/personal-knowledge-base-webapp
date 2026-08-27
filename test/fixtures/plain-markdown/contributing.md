# Contributing

Back [up to the handbook](./README.md).

## Workflow

1. Branch.
2. Change one thing.
3. Run the tests against `test/fixtures/`.
4. Open a pull request.

## Where the fixtures live

See the [reference section](./reference/config-schema.md) for the config file
shape, and the [installation guide](./guides/installation.md) for how to point a
running server at this folder.

```sh
cargo run -p kbview-server -- --root ./test/fixtures/plain-markdown
```

## Style

Prose in Markdown, code in Rust, scripts in Python. Nothing here uses wikilinks.
