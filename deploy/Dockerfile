# Build the frontend, then the binary, then ship only the binary.
# The frontend is embedded into the executable, so the final image has no static
# directory and nothing that can drift out of sync with the code.

FROM node:22-alpine AS web
WORKDIR /build
COPY web/package.json web/package-lock.json* ./
RUN npm ci || npm install
COPY web/ ./
RUN npm run build

FROM rust:1-alpine AS server
RUN apk add --no-cache musl-dev
WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
COPY --from=web /build/dist ./web/dist
RUN cargo build --release --locked -p kbview-server

FROM alpine:3
RUN apk add --no-cache ca-certificates tini && \
    addgroup -g 1033 kbview && \
    adduser -D -u 1033 -G kbview kbview
COPY --from=server /build/target/release/kbview /usr/local/bin/kbview
USER 1033:1033
EXPOSE 4321
ENTRYPOINT ["/sbin/tini", "--", "/usr/local/bin/kbview"]
