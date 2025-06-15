FROM ubuntu:22.04

ENV DEBIAN_FRONTEND=noninteractive
ENV RUSTUP_HOME=/home/rustuser/.rustup
ENV CARGO_HOME=/home/rustuser/.cargo
ENV PATH=$CARGO_HOME/bin:$PATH

RUN apt-get update && apt-get install -y \
    curl \
    wget \
    unzip \
    git \
    build-essential \
    sudo \
    ca-certificates \
    software-properties-common \
    openjdk-11-jdk \
    openjdk-17-jdk \
    openjdk-21-jdk \
    && rm -rf /var/lib/apt/lists/*

RUN useradd -ms /bin/bash rustuser && echo "rustuser ALL=(ALL) NOPASSWD:ALL" >> /etc/sudoers

RUN mkdir -p /home/rustuser/jvms && \
    cp -r /usr/lib/jvm/java-11-openjdk-amd64 /home/rustuser/jvms/jdk-11 && \
    cp -r /usr/lib/jvm/java-17-openjdk-amd64 /home/rustuser/jvms/jdk-17 && \
    cp -r /usr/lib/jvm/java-21-openjdk-amd64 /home/rustuser/jvms/jdk-21 && \
    chown -R rustuser:rustuser /home/rustuser/jvms

USER rustuser
WORKDIR /home/rustuser

RUN curl https://sh.rustup.rs -sSf | bash -s -- -y
ENV PATH="/home/rustuser/.cargo/bin:$PATH"

COPY --chown=rustuser:rustuser . /app
WORKDIR /app 
RUN cargo build --release

ENV RUST_BACKTRACE=1
ENV JAVA_HOME=/home/rustuser/.java/current
ENV PATH=/home/rustuser/.java/current/bin:$PATH

WORKDIR /home/rustuser
RUN mkdir -p /home/rustuser/.config/sjvm && \
  cp /app/tests/it-ubuntu-config.json /home/rustuser/.config/sjvm/sjvm-conf.json
CMD ["/app/target/release/sjvm", "--help"]
