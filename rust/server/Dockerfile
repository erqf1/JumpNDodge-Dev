# JumpNDodge-Server als Docker-Image (z.B. für CasaOS)
# Bauen (im Ordner rust/):  docker build -f server/Dockerfile -t jumpndodge-server .
# Starten: docker run -d --name jumpndodge-server -p 47800:47800/udp -v jnd-data:/data jumpndodge-server
FROM rust:1-slim-bookworm AS build
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY core core
COPY server server
# Das Spiel selbst wird nicht gebraucht - nur Bibliothek + Server bauen
RUN VER=$(sed -n '/^\[workspace.package\]/,/^\[/s/^version = "\(.*\)"/\1/p' Cargo.toml) \
 && printf '[workspace]\nmembers = ["core", "server"]\nresolver = "2"\n\n[workspace.package]\nversion = "%s"\n\n[profile.release]\nopt-level = 3\nlto = "thin"\nstrip = true\n' "$VER" > Cargo.toml \
 && cat Cargo.toml \
 && cargo build --release -p jnd_server

FROM debian:bookworm-slim
LABEL org.opencontainers.image.title="JumpNDodge-Server" \
      org.opencontainers.image.description="Multiplayer-Server für JumpNDodge (Online-VS und Online-Koop)" \
      org.opencontainers.image.licenses="Apache-2.0"
RUN useradd -r -u 1000 -m jnd && mkdir -p /data && chown jnd /data
COPY --from=build /src/target/release/JumpNDodge-Server /usr/local/bin/JumpNDodge-Server
COPY server/LIESMICH.txt /usr/share/doc/jumpndodge-server/LIESMICH.txt
USER jnd
VOLUME ["/data"]
EXPOSE 47800/udp
ENV JND_SERVER_BIND=0.0.0.0
ENTRYPOINT ["JumpNDodge-Server", "--config", "/data/server.json"]
