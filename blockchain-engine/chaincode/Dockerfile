# ------------------------------------------------------------------------------
# ÉTAPE 1 : BUILDER (Image lourde avec Rust à jour)
# ------------------------------------------------------------------------------
# CORRECTIF : On utilise '1-slim-bookworm' pour avoir la dernière version stable (> 1.82)
FROM rust:1-slim-bookworm as builder

WORKDIR /usr/src/raise

# Installation des dépendances système (Protobuf)
RUN apt-get update && apt-get install -y protobuf-compiler libprotobuf-dev && rm -rf /var/lib/apt/lists/*

# Copie de TOUT le dossier engine (Shared + Chaincode)
COPY blockchain-engine ./blockchain-engine

# Compilation en mode Release
RUN cargo install --path blockchain-engine/chaincode

# ------------------------------------------------------------------------------
# ÉTAPE 2 : RUNTIME (Image légère pour la prod)
# ------------------------------------------------------------------------------
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y ca-certificates libssl3 && rm -rf /var/lib/apt/lists/*

RUN useradd -ms /bin/bash chaincode
USER chaincode
WORKDIR /home/chaincode

# Récupération du binaire compilé
COPY --from=builder /usr/local/cargo/bin/raise-chaincode /usr/local/bin/raise-chaincode

EXPOSE 9999

CMD ["raise-chaincode"]