FROM ubuntu:22.04

# Non-interactive apt
ENV DEBIAN_FRONTEND=noninteractive

# Install dependencies
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

# Install Rust
RUN curl https://sh.rustup.rs -sSf | bash -s -- -y
ENV PATH="/root/.cargo/bin:${PATH}"

# Create non-root user
RUN useradd -ms /bin/bash rustuser && echo "rustuser ALL=(ALL) NOPASSWD:ALL" >> /etc/sudoers

# Set up Java alternatives
# RUN update-alternatives --install /usr/bin/java java /usr/lib/jvm/java-11-openjdk-amd64/bin/java 111 \
#  && update-alternatives --install /usr/bin/java java /usr/lib/jvm/java-17-openjdk-amd64/bin/java 117 \
#  && update-alternatives --install /usr/bin/java java /usr/lib/jvm/java-21-openjdk-amd64/bin/java 121

# Copy source code
COPY . /app
WORKDIR /app

# Build the Rust CLI app
RUN cargo build --release

# Switch to non-root user for runtime
USER rustuser
WORKDIR /home/rustuser

# Run built executable
CMD ["/app/target/release/sjvm", "--help"]
