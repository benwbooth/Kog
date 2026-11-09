# Kog's headless server (kog-server) with its web player, for a home server or
# NAS. Built from the self-contained Linux server bundle (packaging/cli), which
# is unpacked into ./server in the build context:
#
#   tar -xzf Kog-*-linux-x86_64-server.tar.gz && mv Kog-*-server server
#   docker build -f packaging/docker/Dockerfile .
#
# The bundle is built on Ubuntu 24.04, so its glibc matches this base.
FROM ubuntu:24.04

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates libasound2t64 \
    && rm -rf /var/lib/apt/lists/*

COPY server/ /opt/kog/

# Kog keeps its settings (server.json: address, port and sign-in), library
# database and cover cache in the XDG folders; mount them to keep them, or to
# share them with the desktop app.
ENV XDG_CONFIG_HOME=/config \
    XDG_DATA_HOME=/data \
    XDG_CACHE_HOME=/cache \
    HOME=/data
RUN mkdir -p /config /data /cache && chmod 777 /config /data /cache

EXPOSE 8420
ENTRYPOINT ["/opt/kog/kog-server"]

LABEL org.opencontainers.image.source="https://github.com/benwbooth/Kog" \
      org.opencontainers.image.description="Kog's headless music server and web player" \
      org.opencontainers.image.licenses="GPL-3.0-or-later"
