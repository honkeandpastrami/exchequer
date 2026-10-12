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

FROM docker.io/library/rust:1.99-alpine AS build
WORKDIR /src
COPY server/ server/
ARG EXCHEQUER_VERSION=0.0.0-dev
# Pure Rust (no TLS, no C libraries). On Alpine the host target is musl, whose
# standard library ships with the image (nothing to download), and musl
# binaries are static by default. sqlx checks queries against the committed
# cache in server/.sqlx, no database needed.
# The build cache is keyed to the toolchain, and locked so concurrent builds
# can't corrupt it. Bump the id to start a cache from scratch.
RUN --mount=type=cache,id=exchequer-cargo-registry,sharing=locked,target=/usr/local/cargo/registry \
    --mount=type=cache,id=exchequer-target-1.99-alpine,sharing=locked,target=/src/server/target \
    cd server \
    && SQLX_OFFLINE=true cargo build --release --locked -p server \
    && cp target/release/exchequer-server /exchequer-server

FROM scratch
COPY --from=build /exchequer-server /exchequer-server
COPY --from=web /src/web/build /web
USER 65532:65532
ENV EXCHEQUER_BIND=0.0.0.0:7438 EXCHEQUER_WEB_DIR=/web
EXPOSE 7438
ENTRYPOINT ["/exchequer-server"]
