# On part de l'image Python (car LEANN a besoin de libs système Python complexes)
FROM python:3.11-slim

# 1. Installation des dépendances système (Compilation C++ et Rust)
RUN apt-get update && apt-get install -y \
    curl build-essential pkg-config libssl-dev \
    git cmake libzmq3-dev libopenblas-dev liblapack-dev \
    && rm -rf /var/lib/apt/lists/*

# 2. Installation de Rust (via rustup)
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
ENV PATH="/root/.cargo/bin:${PATH}"

# 3. Installation de LEANN et des outils Python
RUN pip install uv
# On installe LEANN + sentence-transformers (pour les embeddings)
RUN uv pip install --system "git+https://github.com/yichuan-w/LEANN.git" sentence-transformers

# 4. Préparation du projet Rust
WORKDIR /app
# Copie des fichiers Rust
COPY Cargo.toml .
COPY src ./src

# 5. Compilation du serveur Rust (Release mode)
# PyO3 va détecter l'installation Python du système automatiquement
RUN cargo build --release

# 6. Configuration finale
RUN mkdir -p /data
EXPOSE 8000

# Le conteneur lance directement le binaire Rust !
CMD ["./target/release/leann_rust_wrapper"]