# Running Behind a Reverse Proxy

Two folders deep, so every relative link here has to climb.

- [Guides index](../index.md) — one up
- [Handbook root](../../README.md) — two up
- [Changelog](../../changelog.md) — two up, sibling of the root README
- [HTTP API](../../reference/http-api.md) — two up and back down

## nginx

```nginx
location /kb/ {
    proxy_pass http://127.0.0.1:4321/;
    proxy_set_header Host $host;
    proxy_set_header X-Forwarded-Proto $scheme;
    proxy_set_header X-Forwarded-Prefix /kb;
}
```

## The thing that usually breaks

Relative asset paths. This image is three segments away from the file that
references it:

![Architecture diagram](../../assets/diagram.png)

If it renders here but not on [the guides index](../index.md), the app is
resolving relative paths against the root instead of against the file.
