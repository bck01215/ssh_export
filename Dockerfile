FROM rust:1.52
WORKDIR /opt/remote_exporter
ENV SSH_CONFIG_YAML=/opt/remote_exporter
COPY . .
RUN cargo build --release && cargo install --path .
EXPOSE 7222
CMD ["remote_exporter"]