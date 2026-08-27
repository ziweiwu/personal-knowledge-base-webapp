# Configuration

[Up to the guides index](./index.md)

Configuration is a JSON file. The full shape is in the
[config schema reference](../reference/config-schema.md).

```json
{
  "host": "127.0.0.1",
  "port": 4321,
  "roots": [
    { "id": "handbook", "name": "Handbook", "path": "./test/fixtures/plain-markdown" }
  ]
}
```

## Precedence

| Source | Wins over |
| --- | --- |
| CLI flags | environment |
| Environment | config file |
| Config file | defaults |

See [advanced/reverse-proxy.md](./advanced/reverse-proxy.md) for deployment
behind nginx.
