# Build image
FROM docker.io/clux/muslrust:1.85.1-stable AS build

COPY Cargo.toml Cargo.lock /tmp/
COPY src/ /tmp/src/

RUN cargo install --path /tmp
RUN strip /root/.cargo/bin/azi

# Runtime image
FROM docker.io/alpine:3.20

RUN apk add --no-cache ca-certificates tini && adduser -D -g azi azi

COPY --from=build /root/.cargo/bin/azi /usr/bin/azi

USER azi
WORKDIR /home/azi
ENTRYPOINT [ "/sbin/tini", "--", "/usr/bin/azi" ]
CMD []
