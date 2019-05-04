# Build image
FROM ekidd/rust-musl-builder:1.34.1 AS build

COPY Cargo.toml Cargo.lock /tmp/
COPY src/ /tmp/src/

RUN cargo install --path /tmp && strip /home/rust/.cargo/bin/azi

# Runtime image
FROM alpine:3.9

RUN apk add --no-cache ca-certificates

COPY --from=build /home/rust/.cargo/bin/azi /usr/bin/azi

ENTRYPOINT [ "/usr/bin/azi" ]
CMD []
