# CLI Reference

This file lives in a folder with **no** `index.md` and **no** `README.md`. Opening
`reference/` must produce a generated listing of `cli.md`, `config-schema.md` and
`http-api.md`.

## Flags

| Flag | Default | Meaning |
| --- | --- | --- |
| `--root <PATH>` | — | Folder to serve. Repeatable. |
| `--port <N>` | `4321` | TCP port. |
| `--host <ADDR>` | `127.0.0.1` | Bind address. |
| `--config <FILE>` | `kbview.config.json` | Config file path. |
| `--open` | off | Open a browser on start. |

## Examples

```sh
kbview --root ./notes --root ./docs --port 8080
kbview --config ./kbview.config.json --open
```

Related: [config schema](./config-schema.md), [HTTP API](./http-api.md),
[installation](../guides/installation.md).
