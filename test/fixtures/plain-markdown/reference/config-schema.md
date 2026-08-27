# Config Schema

[Back to the CLI reference](./cli.md) · [handbook root](../README.md)

| Key | Type | Required | Notes |
| --- | --- | :---: | --- |
| `host` | string | no | Defaults to `127.0.0.1`. |
| `port` | integer | no | Defaults to `4321`. |
| `dataDir` | string | no | Where derived state is cached. |
| `roots` | array | yes | At least one entry. |
| `roots[].id` | string | yes | URL-safe, unique. |
| `roots[].name` | string | yes | Shown in the sidebar. |
| `roots[].path` | string | yes | Absolute or relative to the config file. |

## Example

```json
{
  "host": "0.0.0.0",
  "port": 4321,
  "dataDir": "./data",
  "roots": [
    { "id": "vault", "name": "Vault", "path": "./test/fixtures/obsidian-vault" },
    { "id": "handbook", "name": "Handbook", "path": "./test/fixtures/plain-markdown" },
    { "id": "media", "name": "Media", "path": "./test/fixtures/mixed-media" }
  ]
}
```
