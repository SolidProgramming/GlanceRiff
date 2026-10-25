FROM rust:1-slim-bookworm AS build
WORKDIR /src

# Build dependencies in their own layer so code changes don't rebuild them.
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs \
    && cargo build --release --locked \
    && rm -rf src

COPY src ./src
COPY static ./static
RUN touch src/main.rs && cargo build --release --locked

FROM gcr.io/distroless/cc-debian12
COPY --from=build /src/target/release/glanceriff /usr/local/bin/glanceriff
EXPOSE 61208
ENTRYPOINT ["/usr/local/bin/glanceriff"]
CMD ["--web", "--bind", "0.0.0.0:61208"]
