# syntax=docker/dockerfile:1
# Reproducible PowerPC / Wii builds (works on Apple Silicon via qemu/binfmt).
FROM devkitpro/devkitppc:latest

RUN apt-get update && apt-get install -y --no-install-recommends \
        python3 make git \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /src
ENV DEVKITPRO=/opt/devkitpro
ENV DEVKITPPC=/opt/devkitpro/devkitPPC
ENV PATH="${DEVKITPPC}/bin:${PATH}"

CMD ["bash"]
