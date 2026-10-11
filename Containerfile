# The server image: one static binary on an empty base (design §15).
#   podman build -t exchequer-server .
# Runs as an unprivileged user; configure it with the env vars in
# deploy/examples/exchequer-api.container.

FROM docker.io/library/rust:1.99-slim AS build
WORKDIR /src
COPY server/ server/
# Pure Rust (no TLS, no C libraries), so the musl build is fully static. sqlx
# checks queries against the committed cache in server/.sqlx, no database needed.
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/server/target \
    cd server \
    && target="$(uname -m)-unknown-linux-musl" \
    && rustup target add "$target" \
    && SQLX_OFFLINE=true cargo build --release --locked -p server --target "$target" \
    && cp "target/$target/release/exchequer-server" /exchequer-server

FROM scratch
COPY --from=build /exchequer-server /exchequer-server
# Built locally, so it can't be pulled again: a prune can skip it by this label.
LABEL local-build="true"
USER 65532:65532
ENV EXCHEQUER_BIND=0.0.0.0:7438
EXPOSE 7438
ENTRYPOINT ["/exchequer-server"]
