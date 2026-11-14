# JumpNDodge-Server als Docker-Image (z.B. für CasaOS)
# Bauen (im Ordner rust/):  docker build -f server/Dockerfile -t jumpndodge-server .
# Starten: docker run -d --name jumpndodge-server -p 47800:47800/udp -p 47801:47801/tcp -v jnd-data:/data jumpndodge-server
# 47800/udp = Spiel direkt, 47801/tcp = WebSocket (z.B. hinter einem Cloudflare Tunnel)
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
# Startskript: nimmt die neuere Datei (Image oder Auto-Update in /data/bin)
COPY server/docker-entrypoint.sh /usr/local/bin/docker-entrypoint.sh
RUN chmod 755 /usr/local/bin/docker-entrypoint.sh
USER jnd
VOLUME ["/data"]
EXPOSE 47800/udp
EXPOSE 47801/tcp
ENV JND_SERVER_BIND=0.0.0.0 JND_SERVER_DOCKER=1 JND_SERVER_DATA=/data
ENTRYPOINT ["/usr/local/bin/docker-entrypoint.sh"]
