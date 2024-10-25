FROM rust:1.67.0-buster as builder

WORKDIR /usr/src/app
COPY . .

RUN apt update && apt install pkg-config libssl-dev -y
RUN cargo build --release

RUN cp target/release/balance-watcher /balance-watcher

FROM rust:1.67.0-buster
WORKDIR /usr/src/app
COPY --from=builder /balance-watcher /usr/bin/balance-watcher

CMD ["/usr/bin/balance-watcher", "start", "-c", "/usr/src/app/chains.toml"]