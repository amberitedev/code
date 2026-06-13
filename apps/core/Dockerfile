# ============================================================
# Stage 1: Builder
# ============================================================
FROM rust:slim-bookworm AS builder

# Build dependencies:
#   build-essential — gcc/g++/make for C/C++ crates (rusty_v8, portable-pty)
#   pkg-config      — library discovery
#   libssl-dev      — deno_runtime TLS headers (deno links OpenSSL at build time)
#   ca-certificates — HTTPS downloads during build (rusty_v8 pre-built blob)
#   curl            — rustup target installs if needed
RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential \
    pkg-config \
    libssl-dev \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /build

# Copy toolchain pin first — rustup will install 1.90.0 automatically.
COPY rust-toolchain.toml ./

# Copy cargo config (sets SQLX_OFFLINE=true; linker override only applies to MSVC).
COPY .cargo/ ./.cargo/

# Copy manifests for dependency caching.
COPY Cargo.toml Cargo.lock ./

# Copy migrations — required at compile time by sqlx::migrate!().
COPY migrations/ ./migrations/

# Pre-build dependencies using a stub binary so source changes don't invalidate
# the ~10 GB of compiled deps (rusty_v8 alone is ~700 MB).
RUN mkdir -p src && printf 'fn main(){}' > src/main.rs \
    && cargo build --release 2>&1 | tail -5 || true \
    && rm -rf src

# Now build the real source.
COPY src/ ./src/

# Touch main.rs so Cargo sees a source change after the stub trick.
RUN touch src/main.rs && cargo build --release --bin copal

# ============================================================
# Stage 2: Runtime
# ============================================================
FROM debian:bookworm-slim AS runtime

# Runtime libraries:
#   ca-certificates — for HTTPS calls to Supabase, Modrinth, etc.
#   libssl3         — OpenSSL 3 runtime (deno_runtime dynamic links it on Linux)
#   libgcc-s1       — C++ ABI (needed by V8 in some Debian configurations)
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    libssl3 \
    libgcc-s1 \
    && rm -rf /var/lib/apt/lists/*

# Non-root user for security.
RUN useradd -m -u 1001 amberite

COPY --from=builder /build/target/release/copal /usr/local/bin/copal

# Persistent data directory — instances, DB, logs all live here.
RUN mkdir -p /data && chown amberite:amberite /data

USER amberite

VOLUME ["/data"]
EXPOSE 16662

# Defaults — override via env_file or environment: in compose.
ENV AMBERITE_DATA_DIR=/data
ENV PORT=16662
ENV ALLOWED_ORIGIN=https://amberite.dev
# AMBERITE_DEV defaults to false (unset = prod mode).

ENTRYPOINT ["copal"]
CMD ["run"]
