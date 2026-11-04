FROM debian:trixie-slim
ENV DEBIAN_FRONTEND=noninteractive
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates curl git sudo procps psmisc openssh-client file \
    build-essential pkg-config libxkbcommon-x11-dev libfreetype-dev libfontconfig-dev libwayland-dev \
    libvulkan1 mesa-vulkan-drivers xvfb xauth x11-utils xdotool imagemagick openbox weston \
    dbus dbus-x11 libnotify-bin xdg-utils sound-theme-freedesktop chromium \
    fonts-dejavu fonts-noto-core fonts-noto-mono \
    && rm -rf /var/lib/apt/lists/*
RUN useradd -m -s /bin/bash dev && echo 'dev ALL=(ALL) NOPASSWD:ALL' > /etc/sudoers.d/dev \
    && mkdir -p /target && chown dev:dev /target
USER dev
ENV PATH=/home/dev/.cargo/bin:/home/dev/.local/bin:$PATH CARGO_TARGET_DIR=/target DISPLAY=:99
RUN curl -fsSL https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain none \
    && curl -fsSL https://usecornercase.dev/install.sh | sh
WORKDIR /work
