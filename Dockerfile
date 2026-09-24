# Alpine is small and comes with MUSL out of the box, which makes it a great
# candidate for our builder stage.
FROM rust:1-alpine AS build

RUN apk add --no-cache build-base pkgconf openssl-dev ca-certificates perl
RUN rustup target add wasm32-unknown-unknown \
    && cargo install cargo-leptos --locked

WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY src/ src/
COPY assets/ assets/
COPY style.css ./

RUN cargo leptos build --release


# We build static rust binaries which allow us to use a scratch runtime base
# and thus greatly simplify but also limit our runtime environment.
FROM scratch

COPY --from=build /src/target/release/webdash-rs /webdash-rs
COPY --from=build /src/target/site /site
COPY --from=build /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/ca-certificates.crt

ENV LEPTOS_SITE_ROOT=/site \
    LEPTOS_SITE_PKG_DIR=pkg \
    LEPTOS_OUTPUT_NAME=webdash-rs \
    DASHBOARD_LISTEN=0.0.0.0:3000 \
    SSL_CERT_FILE=/etc/ssl/certs/ca-certificates.crt

EXPOSE 3000
ENTRYPOINT ["/webdash-rs"]
