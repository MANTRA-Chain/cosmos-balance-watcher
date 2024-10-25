FROM rust:1.82-slim-bullseye as builder

WORKDIR /usr/src/app
COPY . .

RUN apt update && apt install pkg-config libssl-dev -y
RUN cargo build --release

RUN cp target/release/balance-watcher /balance-watcher

FROM rust:1.82-slim-bullseye
WORKDIR /usr/src/app
COPY --from=builder /balance-watcher /usr/bin/balance-watcher

ENTRYPOINT ["/usr/bin/balance-watcher", "start"]
