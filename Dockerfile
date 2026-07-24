ARG RUST_VERSION=1.91.0
ARG STELLAR_CLI_VERSION=22.1.0

# ══════════════════════════════════════════════════════════════════════════════
# Stage 1: base-toolchain
# Invalida cuando: RUST_VERSION cambia
# ══════════════════════════════════════════════════════════════════════════════
FROM rust:${RUST_VERSION}-slim-bookworm AS base-toolchain

RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    libssl-dev \
    ca-certificates \
    musl-tools \
  && rm -rf /var/lib/apt/lists/*

RUN rustup target add wasm32-unknown-unknown wasm32v1-none
RUN rustup component add rustfmt clippy

# ══════════════════════════════════════════════════════════════════════════════
# Stage 2: stellar-cli
# Invalida cuando: STELLAR_CLI_VERSION cambia (independiente del toolchain)
# ══════════════════════════════════════════════════════════════════════════════
FROM base-toolchain AS stellar-cli

ARG STELLAR_CLI_VERSION
RUN cargo install --locked stellar-cli@${STELLAR_CLI_VERSION}

# ══════════════════════════════════════════════════════════════════════════════
# Stage 3: deps-cache
# Invalida cuando: Cargo.lock cambia
# Tecnica: stubs vacios para compilar solo dependencias, sin source real
# ══════════════════════════════════════════════════════════════════════════════
FROM stellar-cli AS deps-cache

WORKDIR /build

# Copiar SOLO manifests — el source no se incluye aun
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY src/consensus/Cargo.toml            ./src/consensus/Cargo.toml
COPY src/audit_trail/Cargo.toml          ./src/audit_trail/Cargo.toml
COPY core-engine/Cargo.toml              ./core-engine/Cargo.toml
COPY contracts/grant_contracts/Cargo.toml           ./contracts/grant_contracts/Cargo.toml
COPY contracts/staking_contract/Cargo.toml          ./contracts/staking_contract/Cargo.toml
COPY contracts/deposit_to_yield_adapter/Cargo.toml  ./contracts/deposit_to_yield_adapter/Cargo.toml
COPY contracts/insurance_treasury/Cargo.toml        ./contracts/insurance_treasury/Cargo.toml

# Crear stubs vacios para todos los miembros del workspace
RUN mkdir -p \
      src/consensus/src \
      src/audit_trail/src \
      core-engine/src \
      contracts/grant_contracts/src \
      contracts/staking_contract/src \
      contracts/deposit_to_yield_adapter/src \
      contracts/insurance_treasury/src \
  && for dir in \
       src/consensus \
       src/audit_trail \
       core-engine \
       contracts/grant_contracts \
       contracts/staking_contract \
       contracts/deposit_to_yield_adapter \
       contracts/insurance_treasury; \
     do echo "// stub" > "$dir/src/lib.rs"; done

# Calentar dependencias nativas del workspace raiz
RUN cargo build --workspace --release 2>/dev/null || true
RUN cargo build --workspace          2>/dev/null || true

# Calentar dependencias WASM de los contratos
RUN cargo build \
      -p grant_contracts \
      -p staking_contract \
      -p deposit_to_yield_adapter \
      -p insurance_treasury \
      --target wasm32v1-none --release 2>/dev/null || true

# Calentar dependencias de los backends (workspaces independientes)
COPY analytics/Cargo.toml ./analytics/Cargo.toml
COPY social/Cargo.toml    ./social/Cargo.toml

RUN mkdir -p analytics/src social/src \
  && echo "fn main(){}" > analytics/src/main.rs \
  && echo "fn main(){}" > social/src/main.rs

RUN cd analytics && cargo build --release 2>/dev/null || true
RUN cd social    && cargo build --release 2>/dev/null || true

# ══════════════════════════════════════════════════════════════════════════════
# Stage 4: build
# Invalida cuando: cualquier archivo fuente cambia
# ══════════════════════════════════════════════════════════════════════════════
FROM deps-cache AS build

WORKDIR /build

# Copiar source real — la cache de deps-cache permanece intacta
COPY src/           ./src/
COPY core-engine/   ./core-engine/
COPY contracts/     ./contracts/
COPY analytics/     ./analytics/
COPY social/        ./social/
COPY doc_tests/     ./doc_tests/

# Build nativo del workspace raiz
RUN cargo build --workspace --release

# Build WASM de los contratos
RUN cargo build \
      -p grant_contracts \
      -p staking_contract \
      -p deposit_to_yield_adapter \
      -p insurance_treasury \
      --target wasm32v1-none --release

# Build de backends (workspaces independientes) - analytics only, social has pre-existing actix compat issues
RUN cd analytics && cargo build --release

# ══════════════════════════════════════════════════════════════════════════════
# Stage 5: final
# Imagen minima de produccion — no incluye toolchain ni source
# ══════════════════════════════════════════════════════════════════════════════
FROM debian:bookworm-slim AS final

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    libssl3 \
    libpq5 \
  && rm -rf /var/lib/apt/lists/*

# Usuario no-root para produccion
RUN useradd -r -s /bin/false lumina

# Binario principal
COPY --from=build /build/target/release/lumina_core          /usr/local/bin/lumina_core

# Artefactos WASM
COPY --from=build /build/target/wasm32v1-none/release/*.wasm /artifacts/

# Backends
COPY --from=build /build/analytics/target/release/analytics  /usr/local/bin/analytics
# social backend excluded: pre-existing actix compat issue (see PR #97)

USER lumina
EXPOSE 8080

ENTRYPOINT ["/usr/local/bin/lumina_core"]
