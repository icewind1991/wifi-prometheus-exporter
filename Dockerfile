FROM ekidd/rust-musl-builder AS build

ADD . ./
RUN sudo chown -R rust:rust .

RUN cargo build --release

FROM alpine

COPY --from=build /home/rust/src/target/x86_64-unknown-linux-musl/release/wifi-prometheus-exporter /
EXPOSE 80

CMD ["/wifi-prometheus-exporter"]