FROM node:lts AS frontend
WORKDIR /app
COPY package*.json ./
RUN npm ci
COPY src/ src/
COPY public/ public/
COPY index.html vite.config.ts tsconfig*.json tailwind.config.js postcss.config.js ./
RUN npm run build:web

FROM rust:1 AS backend
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY crates/ crates/
COPY src-tauri/src/ src-tauri/src/
COPY src-tauri/Cargo.toml src-tauri/
COPY src-tauri/build.rs src-tauri/
COPY --from=frontend /app/dist dist/
RUN cargo build -p session-web --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=backend /app/target/release/session-web /usr/local/bin/
EXPOSE 3000
ENTRYPOINT ["session-web", "--host", "0.0.0.0"]
