FROM rust:1.81 as base
RUN cargo install cargo-chef
WORKDIR /app

FROM chef AS planner
COPY . .
# COPY ./Cargo.toml ./Cargo.lock ./
# COPY ./src ./src
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --recipe-path recipe.json
# --release
COPY . .
RUN cargo build -p otel-collector
# --release
RUN mv ./target/release/otel-collector ./otel-collector

FROM scratch AS runtime
WORKDIR /app
COPY --from=builder /app/otel-collector /usr/local/bin/
ENTRYPOINT ["/usr/local/bin/otel-collector"]
