# RustWops Development/Testing Environment
# Ubuntu 24.04 LTS with systemd support for testing services

FROM ubuntu:24.04

# Avoid prompts during package installation
ENV DEBIAN_FRONTEND=noninteractive

# Install essential packages
RUN apt-get update && apt-get install -y \
    # Build essentials
    build-essential \
    pkg-config \
    libssl-dev \
    # Rust installation dependencies
    curl \
    ca-certificates \
    # Git for version control
    git \
    # Editors and utilities
    vim \
    nano \
    less \
    htop \
    # Network tools
    wget \
    dnsutils \
    iputils-ping \
    net-tools \
    # Process management
    procps \
    # Systemd (for service management testing)
    systemd \
    systemd-sysv \
    # SQLite
    sqlite3 \
    libsqlite3-dev \
    # Sudo for testing
    sudo \
    && rm -rf /var/lib/apt/lists/*

# Install Rust
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
ENV PATH="/root/.cargo/bin:${PATH}"

# Install useful Rust tools
RUN cargo install cargo-watch

# Create working directory
WORKDIR /rustwops

# Remove unnecessary systemd services that cause issues in containers
RUN rm -f /lib/systemd/system/multi-user.target.wants/* \
    /etc/systemd/system/*.wants/* \
    /lib/systemd/system/local-fs.target.wants/* \
    /lib/systemd/system/sockets.target.wants/*udev* \
    /lib/systemd/system/sockets.target.wants/*initctl* \
    /lib/systemd/system/sysinit.target.wants/systemd-tmpfiles-setup* \
    /lib/systemd/system/systemd-update-utmp*

# Volume for systemd cgroups
VOLUME ["/sys/fs/cgroup"]

# Set up systemd as init
STOPSIGNAL SIGRTMIN+3
CMD ["/lib/systemd/systemd"]
