# Module `vpn`

## 🎯 Objectif

Le module **`vpn`** assure la souveraineté des communications de RAISE. Il encapsule la complexité de gestion d'un réseau Mesh P2P basé sur **Innernet** (une surcouche ergonomique à WireGuard).

Contrairement à un VPN classique (Client-Serveur), ce module permet :

1.  Des connexions directes (Peer-to-Peer) entre les instances RAISE.
2.  Une indépendance totale vis-à-vis des fournisseurs cloud.
3.  Une gestion d'état réactive pour l'interface utilisateur.

---

## 🏗️ Architecture du Client

Le struct `InnernetClient` agit comme un **Wrapper de CLI**. Il pilote les exécutables système et maintient un cache de l'état réseau.

```mermaid
graph TD
    UI[Frontend Tauri] -->|Commandes| Client[InnernetClient Rust]

    subgraph System [Système Hôte]
        Client -->|Exec| InnernetCLI[innernet CLI]
        Client -->|Exec| WG[wg CLI]
        Client -->|Exec| Ping[ping]
    end

    InnernetCLI -->|Config interface| Kernel[Interface WireGuard (raise0)]
    WG -->|Lecture stats| Kernel

```

---

## ⚙️ Configuration (`NetworkConfig`)

La configuration définit les paramètres de l'interface réseau virtuelle.

| Champ             | Type     | Description                 | Valeur par défaut         |
| ----------------- | -------- | --------------------------- | ------------------------- |
| `name`            | `String` | Nom du réseau Innernet      | `"raise"`                 |
| `cidr`            | `String` | Plage d'adresses IP du mesh | `"10.42.0.0/16"`          |
| `interface`       | `String` | Nom de l'interface système  | `"raise0"`                |
| `server_endpoint` | `String` | Point d'entrée pour l'init  | `"vpn.raise.local:51820"` |

---

## 📡 Gestion de la Connexion

### Connexion (`connect`)

La méthode `connect()` exécute la commande `innernet up <name>`.

- **Succès** : Met à jour le statut (`connected = true`) et récupère l'IP assignée via `innernet show`.
- **Échec** : Retourne une `VpnError::Connection` avec la sortie d'erreur standard (stderr).

### Déconnexion (`disconnect`)

Exécute `innernet down <name>`, nettoie l'IP et vide la liste des pairs en mémoire.

### Surveillance (`get_status`)

Le statut est protégé par un verrou asynchrone (`Arc<RwLock<NetworkStatus>>`).
Lorsqu'on demande le statut :

1. Si connecté, le client lance `wg show` (WireGuard) en arrière-plan.
2. Il parse la sortie pour mettre à jour la liste des pairs (IP, Handshake, Transfert).
3. Il retourne une copie de l'état courant.

---

## 👥 Gestion des Pairs

Le module interagit directement avec WireGuard pour obtenir des métriques précises sur les autres nœuds du réseau.

### Structure `Peer`

Chaque pair détecté contient :

- **`public_key`** : L'identifiant cryptographique unique.
- **`endpoint`** : L'adresse IP physique (réelle) et le port.
- **`last_handshake`** : Timestamp du dernier contact (prouve la connectivité).
- **`transfer_rx/tx`** : Volume de données échangées (Bytes).

### Diagnostic (`ping_peer`)

Une commande utilitaire `ping -c 1 -W 2 <ip>` permet de vérifier la latence et l'accessibilité d'un pair spécifique depuis l'application.

---

## 🚨 Prérequis Système

Ce module **ne contient pas** le binaire Innernet. Il suppose que l'environnement hôte dispose de :

1. `innernet` (Installé et configuré).
2. `wg` (Outils WireGuard).
3. Privilèges suffisants (souvent `sudo` ou capabilities réseau) pour créer des interfaces.

La méthode `InnernetClient::check_installation()` permet de valider la présence de la CLI au démarrage.

## 🗺️ Roadmap Implémentation

- [x] Wrapper CLI de base (`up`, `down`, `show`).
- [x] Parsing manuel de la sortie `wg show`.
- [x] Gestion d'état Thread-safe.
- [ ] **Implémentation** : `add_peer` pour gérer les invitations (`innernet install`).
- [ ] **Amélioration** : Parsing plus robuste des unités de transfert (GiB, MiB) dans `wg show`.
- [ ] **Sécurité** : Gestion fine des privilèges (polkit/sudo) pour éviter de lancer toute l'app en root.

```

```
