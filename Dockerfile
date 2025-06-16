# Stage 1: Build the application
FROM rust:latest AS builder

RUN apt-get update && apt-get install -y \
    musl-tools \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

RUN rustup target add x86_64-unknown-linux-musl

WORKDIR /usr/src/app

# Сначала копируем только файлы зависимостей
COPY Cargo.toml Cargo.lock ./
RUN mkdir -p src \
    && echo "fn main() {}" > src/main.rs \
    && cargo build --target x86_64-unknown-linux-musl --release \
    && rm -rf src

# Теперь копируем реальные исходники
COPY src ./src
RUN touch src/main.rs \
    && cargo build --target x86_64-unknown-linux-musl --release

# Stage 2: Минимальный образ
FROM scratch AS final

WORKDIR /app
COPY --from=builder /usr/src/app/target/x86_64-unknown-linux-musl/release/replier_bot .

USER 1000:1000
CMD ["./replier_bot"]