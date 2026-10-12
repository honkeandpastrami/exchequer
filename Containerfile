# The server image: one static binary on an empty base, serving the web app's
# static build at / next to the API (design §13, §15).
#   podman build -t exchequer-server .
# Runs as an unprivileged user; configure it with the env vars in
# deploy/examples/exchequer-api.container. The version comes from git tags:
#   podman build --build-arg EXCHEQUER_VERSION="$(deploy/version)" -t exchequer-server .

FROM docker.io/library/node:26-slim AS web
WORKDIR /src/web
COPY web/package.json web/package-lock.json ./
RUN --mount=type=cache,target=/root/.npm npm ci --no-audit --no-fund --loglevel=error
COPY web/ ./
ARG EXCHEQUER_VERSION=0.0.0-dev
RUN npm run build

# Pure Rust (no TLS, no C libraries). On Alpine the host target is musl, whose
# standard library ships with the image (nothing to download), and musl
# binaries are static by default. sqlx checks queries against the committed
# cache in server/.sqlx, no database needed.
FROM docker.io/library/rust:1.99-alpine AS chef
RUN cargo install cargo-chef --version 0.1.78 --locked
WORKDIR /src/server
ENV SQLX_OFFLINE=true

# The dependencies, from Cargo.toml and Cargo.lock alone (cargo-chef's
# "recipe"), so the layer that compiles them is reused, locally and from CI's
# layer cache, until those change.
FROM chef AS plan
COPY server/ ./
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS build
COPY --from=plan /src/server/recipe.json recipe.json
RUN cargo chef cook --release --locked -p server --recipe-path recipe.json
COPY server/ ./
# After the dependencies: a new version mustn't rebuild them.
ARG EXCHEQUER_VERSION=0.0.0-dev
RUN cargo build --release --locked -p server \
    && cp target/release/exchequer-server /exchequer-server

FROM scratch
COPY --from=build /exchequer-server /exchequer-server
COPY --from=web /src/web/build /web
USER 65532:65532
ENV EXCHEQUER_BIND=0.0.0.0:7438 EXCHEQUER_WEB_DIR=/web
EXPOSE 7438
ENTRYPOINT ["/exchequer-server"]
