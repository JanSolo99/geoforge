# Deliberately no GDAL, PROJ package, compiler or Rust toolchain.
FROM ubuntu:24.04
RUN apt-get update && apt-get install -y --no-install-recommends \
    python3 libstdc++6 libsqlite3-0 ca-certificates \
    && rm -rf /var/lib/apt/lists/*
USER 65534:65534
WORKDIR /tmp
ENTRYPOINT ["python3"]
