FROM ubuntu:22.04

ENV DEBIAN_FRONTEND=noninteractive
ENV RUSTUP_HOME=/home/rustuser/.rustup
ENV CARGO_HOME=/home/rustuser/.cargo
ENV PATH=$CARGO_HOME/bin:$PATH
ENV RUST_BACKTRACE=1
ENV JAVA_HOME=/home/rustuser/.java/current
ENV PATH=$JAVA_HOME/bin:$PATH

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

RUN echo "openjdk" > /home/rustuser/jvms/jdk-11/.sjvm-vendor \
    && echo "openjdk" > /home/rustuser/jvms/jdk-17/.sjvm-vendor \
    && echo "openjdk" > /home/rustuser/jvms/jdk-21/.sjvm-vendor \
    && chown rustuser:rustuser /home/rustuser/jvms/jdk-11/.sjvm-vendor \
    && chown rustuser:rustuser /home/rustuser/jvms/jdk-17/.sjvm-vendor \
    && chown rustuser:rustuser /home/rustuser/jvms/jdk-21/.sjvm-vendor

# Create local JDK archives for E2E tests (bypasses network in test env)
RUN mkdir -p /home/rustuser/jdk-archives \
    && tar -czf /home/rustuser/jdk-archives/jdk-openjdk-11.tar.gz -C /home/rustuser/jvms jdk-11 \
    && tar -czf /home/rustuser/jdk-archives/jdk-openjdk-17.tar.gz -C /home/rustuser/jvms jdk-17 \
    && tar -czf /home/rustuser/jdk-archives/jdk-openjdk-21.tar.gz -C /home/rustuser/jvms jdk-21 \
    && cp -r /home/rustuser/jvms/jdk-21 /home/rustuser/jvms/jdk-graalvm-21 \
    && echo "graalvm" > /home/rustuser/jvms/jdk-graalvm-21/.sjvm-vendor \
    && tar -czf /home/rustuser/jdk-archives/jdk-graalvm-21.tar.gz -C /home/rustuser/jvms jdk-graalvm-21 \
    && rm -rf /home/rustuser/jvms/jdk-graalvm-21 \
    && chown -R rustuser:rustuser /home/rustuser/jdk-archives

USER rustuser
WORKDIR /home/rustuser

RUN curl https://sh.rustup.rs -sSf | bash -s -- -y
ENV PATH="/home/rustuser/.cargo/bin:$PATH"

COPY --chown=rustuser:rustuser . /app
WORKDIR /app 
