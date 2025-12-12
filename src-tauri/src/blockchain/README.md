# 🔗 Module `blockchain`

## Vue d'Ensemble

Le module **`blockchain`** de GenAptitude intègre deux technologies clés pour assurer la **traçabilité réglementaire** et la **souveraineté des communications** :

1. **Hyperledger Fabric** : Blockchain privée pour l'immuabilité des décisions d'architecture
2. **Innernet VPN** : Mesh VPN basé sur WireGuard pour des communications souveraines et sécurisées

Ce module constitue le socle de confiance et de sécurité de la plateforme, permettant une collaboration distribuée tout en maintenant un audit trail complet et des communications chiffrées end-to-end.

### Caractéristiques Principales

**Hyperledger Fabric** :

- ✅ Client léger gRPC pour Tauri
- ✅ Gestion d'identité MSP (Membership Service Provider)
- ✅ Soumission de transactions (submit)
- ✅ Requêtes en lecture seule (query)
- ✅ Récupération d'historique (GetHistoryForKey)
- ⚙️ Chaincode dédié : `arcadia-chaincode`

**Innernet VPN** :

- ✅ Mesh VPN WireGuard simplifié
- ✅ Connexion/déconnexion automatique
- ✅ Gestion des peers
- ✅ Statut réseau en temps réel
- ✅ Ping et diagnostic
- ⚙️ Interface réseau : `genaptitude0`

---

## 🏗️ Architecture Générale

### Structure Modulaire

```
blockchain/
├── mod.rs                    # Point d'entrée Tauri principal
├── fabric/
│   ├── mod.rs                 # Exports publics
│   └── client.rs              # Client Hyperledger Fabric
└── vpn/
    ├── mod.rs                 # Exports publics
    └── innernet_client.rs     # Client Innernet VPN
```

### Intégration Tauri

```
┌─────────────────────────────────────────────────────────────┐
│                     Frontend (TypeScript/React)              │
│                  Composants UI pour VPN et Blockchain        │
└──────────────────────┬───────────────────────────────────────┘
                       │ IPC (Tauri Commands)
                       ▼
┌─────────────────────────────────────────────────────────────┐
│                     Tauri Backend (Rust)                     │
│  ┌──────────────────────────┬───────────────────────────┐   │
│  │   Fabric Commands        │    VPN Commands           │   │
│  │  • record_decision       │   • vpn_connect           │   │
│  │  • verify_decision       │   • vpn_disconnect        │   │
│  │  • query_history         │   • vpn_get_status        │   │
│  │  • record_snapshot       │   • vpn_list_peers        │   │
│  └──────────┬───────────────┴──────────┬────────────────┘   │
│             ▼                          ▼                     │
│  ┌──────────────────┐      ┌──────────────────────┐         │
│  │  FabricClient    │      │  InnernetClient      │         │
│  │  (Arc<RwLock>)   │      │  (Arc<RwLock>)       │         │
│  └──────────┬───────┘      └──────────┬───────────┘         │
└─────────────┼──────────────────────────┼───────────────────┘
              │                          │
              ▼                          ▼
┌──────────────────────┐      ┌──────────────────────┐
│  Hyperledger Fabric  │      │  Innernet/WireGuard  │
│  (Network gRPC)      │      │  (System Commands)   │
└──────────────────────┘      └──────────────────────┘
```

### Flux de Données

**Traçabilité Blockchain** :

```
Décision Architecture (UI)
    ↓
Tauri Command: record_decision()
    ↓
FabricClient::submit_transaction()
    ├─ Signature avec identité MSP
    ├─ Sérialisation JSON
    └─ gRPC → Fabric Peer
    ↓
Chaincode: arcadia-chaincode
    ├─ Validation
    ├─ Consensus (RAFT/Kafka)
    └─ Commit dans le ledger
    ↓
TransactionResult
    ├─ transaction_id (UUID)
    ├─ status (VALID/INVALID)
    ├─ payload (réponse chaincode)
    └─ timestamp
```

**Connexion VPN** :

```
UI: Demande de connexion
    ↓
Tauri Command: vpn_connect()
    ↓
InnernetClient::connect()
    ├─ Exécution: innernet up genaptitude
    ├─ WireGuard interface setup
    ├─ Handshake avec peers
    └─ Attribution IP (10.42.x.x/16)
    ↓
NetworkStatus::connected = true
    ├─ interface: genaptitude0
    ├─ ip_address: 10.42.1.x
    └─ peers: [...]
```

---

## 📚 Modules Détaillés

### 1. Module Principal (`mod.rs`)

**Responsabilité** : Point d'entrée de l'application Tauri, orchestration des services.

#### `AppConfig`

Configuration globale de l'application.

```rust
#[derive(Debug, serde::Deserialize)]
struct AppConfig {
    fabric: FabricConfig,
    vpn: NetworkConfig,
    auto_connect_vpn: bool,
    log_level: String,
}
```

**Champs** :

| Champ              | Type            | Description                                     | Défaut                     |
| ------------------ | --------------- | ----------------------------------------------- | -------------------------- |
| `fabric`           | `FabricConfig`  | Configuration Hyperledger Fabric                | `FabricConfig::default()`  |
| `vpn`              | `NetworkConfig` | Configuration réseau Innernet                   | `NetworkConfig::default()` |
| `auto_connect_vpn` | `bool`          | Connexion auto au VPN au démarrage              | `true`                     |
| `log_level`        | `String`        | Niveau de logging (trace/debug/info/warn/error) | `"info"`                   |

#### Fonction `init_logging()`

Initialise le système de logging avec `tracing`.

```rust
fn init_logging(level: &str) {
    let filter = EnvFilter::try_new(level)
        .unwrap_or_else(|_| EnvFilter::new("info"));

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(true)
        .with_thread_ids(true)
        .with_line_number(true)
        .init();
}
```

**Fonctionnalités** :

- Filtrage par niveau (trace, debug, info, warn, error)
- Affichage du module source (`with_target`)
- Affichage des IDs de threads (`with_thread_ids`)
- Affichage des numéros de ligne (`with_line_number`)

#### Fonction `load_config()`

Charge la configuration depuis un fichier (TODO).

```rust
fn load_config() -> AppConfig {
    // TODO: Charger depuis config.toml ou .env
    let config = AppConfig::default();

    tracing::info!("Configuration loaded:");
    tracing::info!("  - Fabric endpoint: {}", config.fabric.endpoint);
    tracing::info!("  - VPN network: {}", config.vpn.name);

    config
}
```

**Évolution prévue** :

- Lecture depuis `config.toml`
- Variables d'environnement `.env`
- Configuration par projet Tauri

#### Point d'Entrée `mod()`

```rust
#[tokio::mod]
async fn main() {
    let config = load_config();
    init_logging(&config.log_level);

    // Initialisation des clients
    let fabric_client = FabricClient::new(config.fabric.clone());
    let vpn_client = InnernetClient::new(config.vpn.clone());

    // Vérification Innernet
    match InnernetClient::check_installation() {
        Ok(version) => tracing::info!("Innernet found: {}", version),
        Err(e) => tracing::warn!("Innernet not available: {}", e),
    }

    // Lancement Tauri
    tauri::Builder::default()
        .manage(fabric_client)      // État partagé
        .manage(vpn_client)          // État partagé
        .invoke_handler(...)         // Handlers de commandes
        .setup(|app| {
            // Auto-connect VPN si configuré
            if config.auto_connect_vpn {
                // Spawn async task
            }
            Ok(())
        })
        .on_window_event(|event| {
            // Cleanup VPN lors de la fermeture
        })
        .run(tauri::generate_context!())
        .expect("error running tauri");
}
```

**Cycle de vie** :

1. **Démarrage**

   - Chargement config
   - Init logging
   - Création clients (Fabric + VPN)
   - Vérification installation Innernet

2. **Setup Tauri**

   - Enregistrement états partagés (`manage`)
   - Enregistrement commandes IPC
   - Hook setup : auto-connect VPN
   - Health check système

3. **Runtime**

   - Réception commandes UI
   - Exécution async sur clients
   - Logs et monitoring

4. **Shutdown**
   - Interception `CloseRequested`
   - Déconnexion VPN propre
   - Cleanup ressources

#### Commandes Tauri Enregistrées

**Fabric** :

- `record_decision` : Enregistre une décision d'architecture
- `verify_decision` : Vérifie l'intégrité d'une décision
- `query_decision_history` : Récupère l'historique des décisions
- `record_model_snapshot` : Sauvegarde un snapshot de modèle

**VPN** :

- `vpn_connect` : Connexion au mesh VPN
- `vpn_disconnect` : Déconnexion du mesh
- `vpn_get_status` : Récupère le statut réseau
- `vpn_list_peers` : Liste les peers connectés
- `vpn_add_peer` : Ajoute un peer via invitation
- `vpn_ping_peer` : Ping un peer spécifique
- `vpn_check_installation` : Vérifie l'installation Innernet

**Système** :

- `get_system_health` : Récupère la santé du système
- `compute_model_hash` : Calcule le hash d'un modèle

---

### 2. Module Hyperledger Fabric (`fabric/`)

**Responsabilité** : Interaction avec le réseau Hyperledger Fabric pour la traçabilité immuable.

#### `FabricConfig`

Configuration du client Fabric.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FabricConfig {
    pub endpoint: String,        // URL du peer gRPC
    pub msp_id: String,          // ID du MSP
    pub channel_name: String,    // Nom du channel
    pub chaincode_name: String,  // Nom du chaincode
    pub tls_enabled: bool,       // TLS activé
}
```

**Configuration par défaut** :

```rust
impl Default for FabricConfig {
    fn default() -> Self {
        Self {
            endpoint: "grpc://localhost:7051".to_string(),
            msp_id: "GenAptitudeMSP".to_string(),
            channel_name: "genaptitude-channel".to_string(),
            chaincode_name: "arcadia-chaincode".to_string(),
            tls_enabled: false,
        }
    }
}
```

#### `FabricClient`

Client principal pour interagir avec Fabric.

```rust
pub struct FabricClient {
    config: FabricConfig,
    identity: Arc<RwLock<Option<Identity>>>,
}
```

**État interne** :

- `config` : Configuration réseau
- `identity` : Identité MSP chargée (certificat + clé privée)
  - Protégée par `Arc<RwLock>` pour concurrence
  - `None` tant que non chargée

#### `Identity`

Identité MSP pour signer les transactions.

```rust
#[derive(Debug, Clone)]
pub struct Identity {
    pub msp_id: String,         // "GenAptitudeMSP"
    pub certificate: Vec<u8>,   // Certificat X.509 PEM
    pub private_key: Vec<u8>,   // Clé privée ECDSA PEM
}
```

**Chargement d'identité** :

```rust
pub async fn load_identity(&self, cert_path: &str, key_path: &str) -> Result<()> {
    let certificate = fs::read(cert_path).await?;
    let private_key = fs::read(key_path).await?;

    let identity = Identity {
        msp_id: self.config.msp_id.clone(),
        certificate,
        private_key,
    };

    *self.identity.write().await = Some(identity);
    Ok(())
}
```

**Exemple d'utilisation** :

```rust
let fabric_client = FabricClient::new(config);

// Charger l'identité depuis le crypto-config
fabric_client.load_identity(
    "crypto-config/peerOrganizations/genaptitude/users/Admin@genaptitude/msp/signcerts/Admin@genaptitude-cert.pem",
    "crypto-config/peerOrganizations/genaptitude/users/Admin@genaptitude/msp/keystore/priv_sk"
).await?;
```

#### `TransactionResult`

Résultat d'une transaction soumise.

```rust
#[derive(Debug, Serialize, Deserialize)]
pub struct TransactionResult {
    pub transaction_id: String,  // UUID de la transaction
    pub status: String,          // "VALID" ou "INVALID"
    pub payload: Vec<u8>,        // Réponse du chaincode
    pub timestamp: i64,          // Timestamp Unix
}
```

#### `FabricError`

Erreurs du client Fabric.

```rust
#[derive(Debug, thiserror::Error)]
pub enum FabricError {
    #[error("Connection error: {0}")]
    Connection(String),

    #[error("Transaction error: {0}")]
    Transaction(String),

    #[error("Identity error: {0}")]
    Identity(String),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}
```

#### Méthodes Principales

##### `submit_transaction()`

Soumet une transaction au réseau Fabric.

```rust
pub async fn submit_transaction(
    &self,
    function: &str,
    args: Vec<Vec<u8>>,
) -> Result<TransactionResult>
```

**Paramètres** :

- `function` : Nom de la fonction du chaincode à invoquer
- `args` : Arguments encodés en bytes

**Processus** :

```
1. Vérifier qu'une identité est chargée
2. Générer un UUID de transaction
3. Créer la proposition de transaction (Proposal)
4. Signer avec la clé privée MSP
5. Envoyer au peer via gRPC (EndorseProposal)
6. Recevoir l'endorsement
7. Soumettre à l'orderer (BroadcastTransaction)
8. Attendre la confirmation de commit
9. Retourner TransactionResult
```

**Exemple d'utilisation** :

```rust
let decision_json = serde_json::to_vec(&decision)?;

let result = fabric_client.submit_transaction(
    "RecordDecision",
    vec![decision_json]
).await?;

println!("Transaction ID: {}", result.transaction_id);
println!("Status: {}", result.status);
```

**Statut actuel** : ⚠️ Implémentation placeholder (TODO gRPC)

##### `query_transaction()`

Effectue une requête en lecture seule (query).

```rust
pub async fn query_transaction(
    &self,
    function: &str,
    args: Vec<Vec<u8>>
) -> Result<Vec<u8>>
```

**Différences avec `submit_transaction`** :

- ✅ Pas de consensus requis
- ✅ Pas d'écriture dans le ledger
- ✅ Réponse immédiate depuis l'état du peer
- ❌ Pas d'immuabilité garantie

**Exemple d'utilisation** :

```rust
let decision_id = b"decision-12345".to_vec();

let payload = fabric_client.query_transaction(
    "GetDecision",
    vec![decision_id]
).await?;

let decision: Decision = serde_json::from_slice(&payload)?;
```

##### `get_history()`

Récupère l'historique complet d'une clé.

```rust
pub async fn get_history(&self, key: &str) -> Result<Vec<TransactionResult>>
```

**Utilité** :

- Audit trail complet d'une décision
- Traçabilité réglementaire
- Détection de modifications non autorisées

**Exemple d'utilisation** :

```rust
let history = fabric_client.get_history("decision-12345").await?;

for tx in history {
    println!("TX {}: {} at {}",
        tx.transaction_id,
        tx.status,
        tx.timestamp
    );
}
```

**Implémentation** : Utilise `GetHistoryForKey` de Fabric qui retourne toutes les versions d'une clé avec leurs transactions associées.

#### Tests Unitaires

```rust
#[tokio::test]
async fn test_fabric_client_creation() {
    let config = FabricConfig::default();
    let client = FabricClient::new(config);

    assert!(client.identity.read().await.is_none());
}

#[tokio::test]
async fn test_transaction_submission() {
    let config = FabricConfig::default();
    let client = FabricClient::new(config);

    // Doit échouer sans identité
    let result = client
        .submit_transaction("RecordDecision", vec![b"test".to_vec()])
        .await;

    assert!(result.is_err());
}
```

---

### 3. Module Innernet VPN (`vpn/`)

**Responsabilité** : Gestion du mesh VPN Innernet basé sur WireGuard.

#### `NetworkConfig`

Configuration du réseau VPN.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    pub name: String,           // Nom du réseau
    pub cidr: String,           // Plage IP
    pub server_endpoint: String, // Adresse du coordinateur
    pub interface: String,      // Nom de l'interface
}
```

**Configuration par défaut** :

```rust
impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            name: "genaptitude".to_string(),
            cidr: "10.42.0.0/16".to_string(),
            server_endpoint: "vpn.genaptitude.local:51820".to_string(),
            interface: "genaptitude0".to_string(),
        }
    }
}
```

**Plage IP** :

- Réseau : `10.42.0.0/16` (65 534 adresses)
- Coordinateur : `10.42.0.1`
- Peers : `10.42.1.1` - `10.42.255.254`

#### `Peer`

Représentation d'un peer dans le réseau.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Peer {
    pub name: String,           // Nom du peer
    pub ip: String,             // IP privée (10.42.x.x)
    pub public_key: String,     // Clé publique WireGuard
    pub endpoint: Option<String>, // Endpoint public (IP:port)
    pub last_handshake: Option<i64>, // Dernier handshake (timestamp Unix)
    pub transfer_rx: u64,       // Bytes reçus
    pub transfer_tx: u64,       // Bytes envoyés
}
```

**Exemple de peer** :

```json
{
  "name": "workstation-paris",
  "ip": "10.42.1.15",
  "public_key": "abc123def456...",
  "endpoint": "203.0.113.42:51820",
  "last_handshake": 1700000000,
  "transfer_rx": 1048576,
  "transfer_tx": 2097152
}
```

#### `NetworkStatus`

Statut actuel du réseau VPN.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkStatus {
    pub connected: bool,        // Connecté ou non
    pub interface: String,      // Nom de l'interface
    pub ip_address: Option<String>, // IP assignée
    pub peers: Vec<Peer>,       // Liste des peers
    pub uptime_seconds: Option<u64>, // Durée de connexion
}
```

**Exemple de statut** :

```json
{
  "connected": true,
  "interface": "genaptitude0",
  "ip_address": "10.42.1.15",
  "peers": [
    { "name": "server", "ip": "10.42.0.1", ... },
    { "name": "peer-lyon", "ip": "10.42.1.23", ... }
  ],
  "uptime_seconds": 3600
}
```

#### `VpnError`

Erreurs du client VPN.

```rust
#[derive(Debug, thiserror::Error)]
pub enum VpnError {
    #[error("Connection error: {0}")]
    Connection(String),

    #[error("Command execution error: {0}")]
    CommandExecution(String),

    #[error("Parse error: {0}")]
    Parse(String),

    #[error("Network not configured")]
    NotConfigured,
}
```

#### `InnernetClient`

Client principal pour gérer Innernet.

```rust
pub struct InnernetClient {
    config: NetworkConfig,
    status: Arc<RwLock<NetworkStatus>>,
}
```

**État interne** :

- `config` : Configuration réseau
- `status` : Statut en temps réel (thread-safe)

#### Méthodes Principales

##### `check_installation()`

Vérifie si Innernet est installé sur le système.

```rust
pub fn check_installation() -> Result<String>
```

**Processus** :

```rust
let output = Command::new("innernet")
    .arg("--version")
    .output()?;

if output.status.success() {
    let version = String::from_utf8_lossy(&output.stdout);
    Ok(version.trim().to_string())
} else {
    Err(VpnError::CommandExecution("Innernet not found"))
}
```

**Retour** :

- `Ok("innernet 1.6.1")` si installé
- `Err(...)` si non installé

##### `connect()`

Connexion au réseau mesh.

```rust
pub async fn connect(&self) -> Result<()>
```

**Processus** :

```
1. Exécuter : innernet up genaptitude
2. WireGuard crée l'interface genaptitude0
3. Handshake avec les peers connus
4. Attribution IP depuis le coordinateur
5. Mise à jour du statut (connected = true)
6. Récupération de l'IP assignée
7. Logs de connexion réussie
```

**Exemple de logs** :

```
[INFO] Connecting to Innernet network: genaptitude
[INFO] Successfully connected to genaptitude
[INFO] Assigned IP: 10.42.1.15
```

**Gestion d'erreurs** :

```rust
if !output.status.success() {
    let stderr = String::from_utf8_lossy(&output.stderr);
    return Err(VpnError::Connection(format!("Failed to connect: {}", stderr)));
}
```

##### `disconnect()`

Déconnexion du réseau mesh.

```rust
pub async fn disconnect(&self) -> Result<()>
```

**Processus** :

```
1. Exécuter : innernet down genaptitude
2. WireGuard détruit l'interface genaptitude0
3. Fermeture des connexions aux peers
4. Mise à jour du statut (connected = false)
5. Effacement IP et peers
6. Logs de déconnexion
```

**Exemple de logs** :

```
[INFO] Disconnecting from Innernet network: genaptitude
[INFO] Successfully disconnected from genaptitude
```

##### `get_status()`

Récupère le statut actuel du réseau.

```rust
pub async fn get_status(&self) -> Result<NetworkStatus>
```

**Processus** :

```rust
if !self.status.read().await.connected {
    return Ok(self.status.read().await.clone());
}

// Mettre à jour la liste des peers
if let Ok(peers) = self.fetch_peers().await {
    let mut status = self.status.write().await;
    status.peers = peers;
}

Ok(self.status.read().await.clone())
```

**Retourne** : Clone du `NetworkStatus` actuel avec peers mis à jour.

##### `list_peers()`

Liste tous les peers du réseau.

```rust
pub async fn list_peers(&self) -> Result<Vec<Peer>>
```

**Implémentation** :

```rust
pub async fn list_peers(&self) -> Result<Vec<Peer>> {
    self.fetch_peers().await
}
```

Appelle `fetch_peers()` qui utilise `wg show` pour récupérer les informations WireGuard.

##### `add_peer()`

Ajoute un nouveau peer via un code d'invitation.

```rust
pub async fn add_peer(&self, invitation_code: &str) -> Result<String>
```

**Processus prévu** :

```
1. Réception d'un fichier d'invitation (.toml)
2. Exécution : innernet install invitation.toml
3. Configuration automatique du peer
4. Ajout dans la liste des peers autorisés
5. Handshake initial
```

**Statut** : ⚠️ TODO implémentation

**Format d'invitation (exemple)** :

```toml
[interface]
network_name = "genaptitude"
address = "10.42.1.25/32"
private_key = "..."

[peer]
public_key = "..."
endpoint = "vpn.genaptitude.local:51820"
allowed_ips = "10.42.0.0/16"
```

##### `ping_peer()`

Teste la connectivité avec un peer.

```rust
pub async fn ping_peer(&self, peer_ip: &str) -> Result<bool>
```

**Implémentation** :

```rust
let output = Command::new("ping")
    .args(&["-c", "1", "-W", "2", peer_ip])
    .output()?;

Ok(output.status.success())
```

**Paramètres** :

- `-c 1` : Un seul ping
- `-W 2` : Timeout de 2 secondes
- `peer_ip` : IP du peer (ex: `10.42.1.23`)

#### Méthodes Internes

##### `run_command()`

Exécute une commande Innernet.

```rust
fn run_command(&self, args: &[&str]) -> Result<Output> {
    Command::new("innernet")
        .args(args)
        .output()
        .map_err(|e| VpnError::CommandExecution(e.to_string()))
}
```

**Exemples d'utilisation** :

```rust
self.run_command(&["up", "genaptitude"])?;
self.run_command(&["down", "genaptitude"])?;
self.run_command(&["show", "genaptitude"])?;
```

##### `get_interface_ip()`

Récupère l'IP assignée à l'interface.

```rust
async fn get_interface_ip(&self) -> Result<String>
```

**Processus** :

```
1. Exécuter : innernet show genaptitude
2. Parser la sortie pour trouver la ligne "ip:"
3. Extraire l'IP (format: "10.42.1.15/24" → "10.42.1.15")
4. Retourner l'IP ou erreur
```

**Exemple de sortie parsée** :

```
interface: genaptitude0, ip: 10.42.1.15/24
endpoint: vpn.genaptitude.local:51820
```

##### `fetch_peers()`

Récupère la liste des peers via WireGuard.

```rust
async fn fetch_peers(&self) -> Result<Vec<Peer>>
```

**Implémentation** :

```rust
let output = Command::new("wg")
    .args(&["show", &self.config.interface])
    .output()?;

let stdout = String::from_utf8_lossy(&output.stdout);
let peers = self.parse_wg_output(&stdout)?;

Ok(peers)
```

**Utilité** :

- Plus fiable que `innernet show`
- Accès direct aux stats WireGuard
- Informations détaillées (handshake, transfer)

##### `parse_wg_output()`

Parse la sortie de `wg show`.

```rust
fn parse_wg_output(&self, output: &str) -> Result<Vec<Peer>>
```

**Format de sortie `wg show`** :

```
interface: genaptitude0
  public key: abc123...
  private key: (hidden)
  listening port: 51820

peer: def456...
  endpoint: 192.168.1.100:51820
  allowed ips: 10.42.1.1/32, 10.42.2.0/24
  latest handshake: 30 seconds ago
  transfer: 1.5 KiB received, 2.3 KiB sent
  persistent keepalive: every 25 seconds

peer: ghi789...
  endpoint: 203.0.113.42:51820
  allowed ips: 10.42.1.23/32
  latest handshake: 2 minutes, 15 seconds ago
  transfer: 512 B received, 1.2 KiB sent
```

**Algorithme de parsing** :

```
1. Initialiser liste peers vide
2. Initialiser current_peer = None
3. Pour chaque ligne :
   a. Si ligne commence par "peer:"
      - Sauvegarder current_peer si existant
      - Créer nouveau peer avec public_key
   b. Si current_peer existe :
      - "endpoint:" → peer.endpoint
      - "allowed ips:" → peer.ip (première IP)
      - "latest handshake:" → peer.last_handshake
      - "transfer:" → peer.transfer_rx/tx (TODO parse)
4. Ajouter le dernier peer
5. Retourner liste
```

**Exemple de peer parsé** :

```rust
Peer {
    name: "unknown",  // Innernet ne fournit pas le nom via wg
    ip: "10.42.1.1",
    public_key: "def456...",
    endpoint: Some("192.168.1.100:51820"),
    last_handshake: Some(1700000000),
    transfer_rx: 0,   // TODO: parser "1.5 KiB"
    transfer_tx: 0,   // TODO: parser "2.3 KiB"
}
```

#### Tests Unitaires

```rust
#[test]
fn test_network_config_default() {
    let config = NetworkConfig::default();
    assert_eq!(config.name, "genaptitude");
    assert_eq!(config.cidr, "10.42.0.0/16");
}

#[tokio::test]
async fn test_innernet_client_creation() {
    let config = NetworkConfig::default();
    let client = InnernetClient::new(config);

    let status = client.status.read().await;
    assert!(!status.connected);
}

#[test]
fn test_parse_wg_output() {
    let config = NetworkConfig::default();
    let client = InnernetClient::new(config);

    let wg_output = r#"
interface: genaptitude0
  public key: abc123...

peer: def456...
  endpoint: 192.168.1.100:51820
  allowed ips: 10.42.1.1/32
  latest handshake: 30 seconds ago
    "#;

    let peers = client.parse_wg_output(wg_output).unwrap();
    assert_eq!(peers.len(), 1);
    assert_eq!(peers[0].ip, "10.42.1.1");
}
```

---

## 🔐 Sécurité et Souveraineté

### Architecture de Confiance

```
┌────────────────────────────────────────────────────┐
│           Souveraineté des Données                 │
│  ┌──────────────────────┬──────────────────────┐  │
│  │  Hyperledger Fabric  │  Innernet VPN        │  │
│  │  • Blockchain privée │  • Mesh P2P chiffré  │  │
│  │  • Consensus RAFT    │  • WireGuard         │  │
│  │  │  • Pas de cloud   │  • Pas de VPN tiers  │  │
│  │  • Audit trail       │  • NAT traversal     │  │
│  └──────────────────────┴──────────────────────┘  │
│                                                     │
│  ┌──────────────────────────────────────────────┐ │
│  │        Traçabilité Réglementaire             │ │
│  │  • Immuabilité des décisions                 │ │
│  │  • Horodatage certifié                       │ │
│  │  • Identités cryptographiques (MSP)          │ │
│  │  • Historique complet (GetHistory)           │ │
│  └──────────────────────────────────────────────┘ │
└────────────────────────────────────────────────────┘
```

### Hyperledger Fabric : Immuabilité et Consensus

**Garanties cryptographiques** :

1. **Identité MSP** : Certificats X.509 pour chaque participant
2. **Signature de transactions** : ECDSA avec clés privées
3. **Hashing** : SHA-256 pour le ledger
4. **Consensus** : RAFT ou Kafka pour l'ordre des transactions
5. **Chaincode** : Smart contracts en Go/Node.js pour la logique métier

**Flux de confiance** :

```
Décision d'architecture
    ↓
Signature avec clé privée MSP (ECDSA)
    ↓
Endorsement par peers autorisés
    ↓
Consensus sur l'ordre (RAFT)
    ↓
Commit dans le ledger immuable
    ↓
Hash cryptographique du block
    ↓
Vérification ultérieure possible
```

**Cas d'usage GenAptitude** :

- Décisions d'architecture MBSE
- Snapshots de modèles Capella/Arcadia
- Validations réglementaires
- Changements de requirements
- Traçabilité ISO 26262 / DO-178C

### Innernet VPN : Chiffrement et Souveraineté

**Garanties cryptographiques** :

1. **Protocole WireGuard** : ChaCha20-Poly1305 (chiffrement)
2. **Échange de clés** : Curve25519 (ECDH)
3. **Authentification** : BLAKE2s
4. **Clés éphémères** : Rotation automatique
5. **Forward secrecy** : Compromission d'une session ≠ compromission historique

**Modèle de souveraineté** :

```
┌─────────────────────────────────────────┐
│     Coordinateur Innernet (auto-hébergé)│
│     - Attribution IP                    │
│     - Gestion ACL                       │
│     - PAS de routage de trafic          │
└──────────┬──────────────────────────────┘
           │
           ▼
    [Peers en mesh P2P]
    • Paris    ←→  Lyon
    • Lyon     ←→  Toulouse
    • Toulouse ←→  Paris

    → Connexions directes chiffrées
    → Pas de point central de routage
    → NAT traversal automatique
```

**Avantages vs VPN traditionnels** :

| Critère              | VPN Classique             | Innernet              |
| -------------------- | ------------------------- | --------------------- |
| Architecture         | Client-Serveur            | Mesh P2P              |
| Point de défaillance | Oui (serveur central)     | Non (décentralisé)    |
| Routage              | Tout passe par le serveur | Direct peer-to-peer   |
| Performance          | Limitée par le serveur    | Directe entre peers   |
| Souveraineté         | Dépend du fournisseur     | Totale (auto-hébergé) |
| Complexité           | Simple                    | Modérée               |

**Cas d'usage GenAptitude** :

- Collaboration inter-sites (Paris ↔ Lyon ↔ Toulouse)
- Accès distant sécurisé aux modèles
- Communication agents LLM distribués
- Synchronisation bases de données
- Pas de dépendance cloud (AWS, Azure, etc.)

---

## 💻 Utilisation Pratique

### Configuration Initiale

#### 1. Hyperledger Fabric

**Prérequis** :

- Réseau Fabric déployé (peers, orderers, CA)
- Channel créé : `genaptitude-channel`
- Chaincode déployé : `arcadia-chaincode`
- Certificats MSP générés

**Structure crypto-config** :

```
crypto-config/
└── peerOrganizations/
    └── genaptitude/
        ├── peers/
        │   └── peer0.genaptitude/
        ├── users/
        │   └── Admin@genaptitude/
        │       └── msp/
        │           ├── signcerts/
        │           │   └── Admin@genaptitude-cert.pem
        │           └── keystore/
        │               └── priv_sk
        └── msp/
            └── ...
```

**Configuration dans GenAptitude** :

```rust
let fabric_config = FabricConfig {
    endpoint: "grpc://peer0.genaptitude.local:7051".to_string(),
    msp_id: "GenAptitudeMSP".to_string(),
    channel_name: "genaptitude-channel".to_string(),
    chaincode_name: "arcadia-chaincode".to_string(),
    tls_enabled: true,
};

let fabric_client = FabricClient::new(fabric_config);

fabric_client.load_identity(
    "./crypto-config/.../Admin@genaptitude-cert.pem",
    "./crypto-config/.../priv_sk"
).await?;
```

#### 2. Innernet VPN

**Installation Innernet** :

```bash
# Ubuntu/Debian
curl -LO https://github.com/tonarino/innernet/releases/latest/download/innernet_amd64.deb
sudo dpkg -i innernet_amd64.deb

# Vérification
innernet --version
```

**Configuration du coordinateur** (serveur) :

```bash
# Créer le réseau
sudo innernet-server new genaptitude \
    --cidr 10.42.0.0/16 \
    --listen-port 51820 \
    --data-dir /var/lib/innernet-server/genaptitude

# Démarrer le service
sudo systemctl enable innernet-server@genaptitude
sudo systemctl start innernet-server@genaptitude
```

**Génération d'invitation pour un peer** :

```bash
# Sur le serveur
sudo innernet-server add-peer genaptitude \
    --name "workstation-paris" \
    --cidr 10.42.1.0/24 \
    --admin

# Exporter l'invitation
sudo innernet-server export genaptitude \
    --peer workstation-paris \
    > invitation-paris.toml
```

**Installation peer** :

```bash
# Sur le poste client
sudo innernet install invitation-paris.toml

# Connexion manuelle
sudo innernet up genaptitude

# Vérification
sudo innernet show genaptitude
sudo wg show genaptitude0
```

**Configuration dans GenAptitude** :

```rust
let vpn_config = NetworkConfig {
    name: "genaptitude".to_string(),
    cidr: "10.42.0.0/16".to_string(),
    server_endpoint: "vpn.genaptitude.local:51820".to_string(),
    interface: "genaptitude0".to_string(),
};

let vpn_client = InnernetClient::new(vpn_config);

// Auto-connect au démarrage
if auto_connect {
    vpn_client.connect().await?;
}
```

### Commandes Tauri (Frontend → Backend)

#### Fabric : Enregistrer une décision

```typescript
import { invoke } from '@tauri-apps/api/tauri';

interface Decision {
  id: string;
  title: string;
  description: string;
  impact: 'low' | 'medium' | 'high';
  author: string;
  timestamp: number;
}

async function recordDecision(decision: Decision) {
  try {
    const result = await invoke('record_decision', {
      decision: JSON.stringify(decision),
    });

    console.log('Transaction ID:', result.transaction_id);
    console.log('Status:', result.status);

    return result;
  } catch (error) {
    console.error('Failed to record decision:', error);
    throw error;
  }
}
```

#### Fabric : Vérifier une décision

```typescript
async function verifyDecision(decisionId: string) {
  try {
    const isValid = await invoke('verify_decision', {
      decisionId,
    });

    return isValid;
  } catch (error) {
    console.error('Verification failed:', error);
    return false;
  }
}
```

#### Fabric : Historique d'une décision

```typescript
async function getDecisionHistory(decisionId: string) {
  try {
    const history = await invoke('query_decision_history', {
      decisionId,
    });

    // history: TransactionResult[]
    console.log(`Found ${history.length} transactions`);

    return history;
  } catch (error) {
    console.error('Failed to get history:', error);
    return [];
  }
}
```

#### VPN : Connexion

```typescript
async function connectVPN() {
  try {
    await invoke('vpn_connect');
    console.log('VPN connected');

    // Récupérer le statut
    const status = await invoke('vpn_get_status');
    console.log('IP:', status.ip_address);
    console.log('Peers:', status.peers.length);
  } catch (error) {
    console.error('VPN connection failed:', error);
  }
}
```

#### VPN : Déconnexion

```typescript
async function disconnectVPN() {
  try {
    await invoke('vpn_disconnect');
    console.log('VPN disconnected');
  } catch (error) {
    console.error('VPN disconnection failed:', error);
  }
}
```

#### VPN : Statut et peers

```typescript
async function getVPNStatus() {
  try {
    const status = await invoke('vpn_get_status');

    console.log('Connected:', status.connected);
    console.log('Interface:', status.interface);
    console.log('IP:', status.ip_address);
    console.log('Peers:', status.peers);

    return status;
  } catch (error) {
    console.error('Failed to get VPN status:', error);
  }
}

async function listPeers() {
  try {
    const peers = await invoke('vpn_list_peers');

    peers.forEach((peer) => {
      console.log(`${peer.name} (${peer.ip})`);
      console.log(`  Endpoint: ${peer.endpoint}`);
      console.log(`  Last handshake: ${peer.last_handshake}`);
    });

    return peers;
  } catch (error) {
    console.error('Failed to list peers:', error);
    return [];
  }
}
```

#### VPN : Ping un peer

```typescript
async function pingPeer(peerIp: string) {
  try {
    const isReachable = await invoke('vpn_ping_peer', {
      peerIp,
    });

    if (isReachable) {
      console.log(`✓ ${peerIp} is reachable`);
    } else {
      console.log(`✗ ${peerIp} is not reachable`);
    }

    return isReachable;
  } catch (error) {
    console.error(`Failed to ping ${peerIp}:`, error);
    return false;
  }
}
```

---

## 🧪 Tests et Validation

### Tests Unitaires

**Fabric** :

```rust
cargo test --package genaptitude --lib fabric
```

**Tests disponibles** :

- `test_fabric_client_creation` : Création du client
- `test_transaction_submission` : Soumission sans identité (doit échouer)

**VPN** :

```rust
cargo test --package genaptitude --lib vpn
```

**Tests disponibles** :

- `test_network_config_default` : Configuration par défaut
- `test_innernet_client_creation` : Création du client
- `test_parse_wg_output` : Parsing de la sortie WireGuard

### Tests d'Intégration

**Test Fabric end-to-end** :

```rust
#[tokio::test]
async fn test_fabric_full_flow() {
    // 1. Créer le client
    let config = FabricConfig::default();
    let client = FabricClient::new(config);

    // 2. Charger l'identité
    client.load_identity("./test-crypto/cert.pem", "./test-crypto/key.pem")
        .await
        .expect("load identity");

    // 3. Soumettre une transaction
    let decision = json!({
        "id": "decision-test-1",
        "title": "Test Decision",
        "impact": "low"
    });

    let result = client.submit_transaction(
        "RecordDecision",
        vec![serde_json::to_vec(&decision).unwrap()]
    ).await.expect("submit transaction");

    assert_eq!(result.status, "VALID");
    assert!(!result.transaction_id.is_empty());

    // 4. Query la décision
    let payload = client.query_transaction(
        "GetDecision",
        vec![b"decision-test-1".to_vec()]
    ).await.expect("query transaction");

    let retrieved: serde_json::Value = serde_json::from_slice(&payload).unwrap();
    assert_eq!(retrieved["id"], "decision-test-1");
}
```

**Test VPN end-to-end** :

```rust
#[tokio::test]
async fn test_vpn_full_flow() {
    // 1. Vérifier installation
    let version = InnernetClient::check_installation()
        .expect("innernet should be installed");
    println!("Innernet version: {}", version);

    // 2. Créer le client
    let config = NetworkConfig::default();
    let client = InnernetClient::new(config);

    // 3. Connexion
    client.connect().await.expect("connect");

    // 4. Vérifier statut
    let status = client.get_status().await.expect("get status");
    assert!(status.connected);
    assert!(status.ip_address.is_some());

    // 5. Lister peers
    let peers = client.list_peers().await.expect("list peers");
    println!("Found {} peers", peers.len());

    // 6. Déconnexion
    client.disconnect().await.expect("disconnect");

    let status = client.get_status().await.expect("get status");
    assert!(!status.connected);
}
```

### Tests Manuels

**Fabric** :

```bash
# 1. Vérifier le réseau Fabric
docker ps | grep hyperledger

# 2. Tester une transaction via CLI Fabric
peer chaincode invoke \
    -C genaptitude-channel \
    -n arcadia-chaincode \
    -c '{"function":"RecordDecision","Args":["decision-test-1","Test Decision"]}' \
    --waitForEvent

# 3. Query la décision
peer chaincode query \
    -C genaptitude-channel \
    -n arcadia-chaincode \
    -c '{"function":"GetDecision","Args":["decision-test-1"]}'

# 4. Historique
peer chaincode query \
    -C genaptitude-channel \
    -n arcadia-chaincode \
    -c '{"function":"GetHistory","Args":["decision-test-1"]}'
```

**VPN** :

```bash
# 1. Vérifier installation
innernet --version
wg --version

# 2. Connexion manuelle
sudo innernet up genaptitude

# 3. Vérifier interface
ip addr show genaptitude0
sudo wg show genaptitude0

# 4. Lister peers
sudo innernet list genaptitude

# 5. Ping un peer
ping -c 3 10.42.1.1

# 6. Statistiques WireGuard
sudo wg show genaptitude0 transfer

# 7. Déconnexion
sudo innernet down genaptitude
```

---

## 📊 Monitoring et Observabilité

### Logs avec Tracing

**Niveaux de logs** :

| Niveau  | Description                   | Exemple               |
| ------- | ----------------------------- | --------------------- |
| `trace` | Débogage très détaillé        | Parsing de paquets    |
| `debug` | Informations de développement | Appels de fonctions   |
| `info`  | Événements normaux            | Connexion VPN réussie |
| `warn`  | Avertissements                | VPN non installé      |
| `error` | Erreurs                       | Échec de transaction  |

**Configuration** :

```bash
# Tous les logs en debug
RUST_LOG=debug cargo run

# Logs spécifiques au module blockchain
RUST_LOG=genaptitude::fabric=trace,genaptitude::vpn=debug cargo run

# Production : warnings et erreurs uniquement
RUST_LOG=warn cargo run --release
```

**Exemples de logs** :

```
[INFO  genaptitude] Starting GenAptitude v0.1.0
[INFO  genaptitude::fabric] Fabric client initialized
[INFO  genaptitude::vpn] Innernet found: innernet 1.6.1
[INFO  genaptitude::vpn] Connecting to Innernet network: genaptitude
[INFO  genaptitude::vpn] Successfully connected to genaptitude
[INFO  genaptitude::vpn] Assigned IP: 10.42.1.15
[INFO  genaptitude::fabric] Submitting transaction to genaptitude-channel/arcadia-chaincode: RecordDecision with 1 args
[DEBUG genaptitude::vpn] Fetching peers via WireGuard
[DEBUG genaptitude::vpn] Parsed 3 peers from wg output
[WARN  genaptitude::vpn] VPN auto-connect failed: Network not configured
[ERROR genaptitude::fabric] Transaction error: Identity not loaded
```

### Métriques

**Fabric** :

- Nombre de transactions soumises
- Taux de succès/échec
- Latence moyenne
- Taille du ledger

**VPN** :

- Nombre de peers connectés
- Uptime réseau
- Bandwidth total (RX/TX)
- Latence inter-peers
- Handshakes réussis/échoués

**Implémentation future** :

```rust
struct BlockchainMetrics {
    fabric_transactions_total: Counter,
    fabric_transaction_duration: Histogram,
    vpn_peers_connected: Gauge,
    vpn_bytes_transferred: Counter,
}
```

### Health Checks

**Endpoint santé** (commande Tauri) :

```typescript
interface SystemHealth {
  fabric: {
    connected: boolean;
    endpoint: string;
    identity_loaded: boolean;
  };
  vpn: {
    connected: boolean;
    ip_address: string | null;
    peers_count: number;
  };
  tauri: {
    version: string;
    uptime_seconds: number;
  };
}

const health = await invoke('get_system_health');
```

**Implémentation backend** :

```rust
#[tauri::command]
async fn get_system_health(
    fabric: State<'_, FabricClient>,
    vpn: State<'_, InnernetClient>,
) -> Result<SystemHealth, String> {
    let fabric_status = {
        let identity = fabric.identity.read().await;
        FabricStatus {
            connected: true, // TODO: vérifier connexion réelle
            endpoint: fabric.config.endpoint.clone(),
            identity_loaded: identity.is_some(),
        }
    };

    let vpn_status = {
        let status = vpn.get_status().await.map_err(|e| e.to_string())?;
        VpnStatus {
            connected: status.connected,
            ip_address: status.ip_address,
            peers_count: status.peers.len(),
        }
    };

    Ok(SystemHealth {
        fabric: fabric_status,
        vpn: vpn_status,
        tauri: TauriStatus {
            version: env!("CARGO_PKG_VERSION").to_string(),
            uptime_seconds: /* TODO */,
        },
    })
}
```

---

## 🚀 Roadmap et Extensions

### Court Terme

#### Fabric

- [ ] **Implémentation gRPC complète**

  - Utiliser `tonic` pour les appels Fabric
  - Support des propositions de transaction
  - Gestion des endorsements
  - Broadcast à l'orderer

- [ ] **Gestion TLS**

  - Certificats CA pour TLS
  - Validation des certificats peers
  - Mutual TLS (mTLS)

- [ ] **Cache des transactions**
  - Éviter les requêtes répétées
  - Invalidation automatique
  - Persistance locale

#### VPN

- [ ] **Implémentation `add_peer()`**

  - Parser fichiers d'invitation .toml
  - Commande `innernet install`
  - Validation automatique

- [ ] **Parsing complet de `wg show`**

  - Transfer stats (bytes/KiB/MiB/GiB)
  - Timestamps de handshake relatifs
  - Keepalive persistent

- [ ] **UI pour gestion des peers**
  - Invitation QR code
  - Révocation de peers
  - Groupes et ACLs

### Moyen Terme

#### Fabric

- [ ] **Support multi-channels**

  - Gestion de plusieurs channels
  - Switch dynamique
  - Isolation par projet

- [ ] **Queries riches**

  - CouchDB indexes
  - Queries JSON complexes
  - Pagination

- [ ] **Events Fabric**
  - Écoute des événements blockchain
  - Notifications temps réel UI
  - Webhooks

#### VPN

- [ ] **Monitoring avancé**

  - Graphiques de bande passante
  - Historique de connexions
  - Alertes déconnexion

- [ ] **NAT traversal amélioré**

  - Détection automatique NAT
  - STUN/TURN fallback
  - Relais automatiques

- [ ] **Multi-réseaux**
  - Plusieurs réseaux Innernet
  - Switch automatique
  - Routage inter-réseaux

### Long Terme

#### Fabric

- [ ] **Chaincode en WASM**

  - Développement simplifié
  - Portabilité
  - Sandboxing renforcé

- [ ] **Integration Fabric CA**

  - Enrôlement automatique
  - Renouvellement certificats
  - Révocation

- [ ] **Identités multiples**
  - Switch entre identités
  - Rôles et permissions
  - Délégation

#### VPN

- [ ] **Mesh routing intelligent**

  - Découverte automatique de routes
  - Failover automatique
  - Load balancing

- [ ] **Intégration DNS**

  - Résolution de noms locaux
  - Service discovery
  - mDNS/Avahi

- [ ] **Mobile support**
  - Android/iOS clients
  - Roaming support
  - Battery optimization

---

## 🔧 Dépannage

### Fabric

**Problème : "Identity error: No identity loaded"**

```
Cause : Aucune identité MSP n'a été chargée
Solution :
  1. Vérifier que les certificats existent
  2. Appeler load_identity() avec les bons chemins
  3. Vérifier les permissions des fichiers
```

**Problème : "Connection error: Failed to connect to peer"**

```
Cause : Le peer Fabric n'est pas accessible
Solution :
  1. Vérifier que le réseau Fabric est démarré
  2. Tester la connectivité : telnet peer0.genaptitude.local 7051
  3. Vérifier la configuration endpoint dans FabricConfig
  4. Vérifier les logs du peer Fabric
```

**Problème : "Transaction error: ENDORSEMENT_POLICY_FAILURE"**

```
Cause : La policy d'endorsement n'est pas satisfaite
Solution :
  1. Vérifier la policy du chaincode
  2. S'assurer que suffisamment de peers endorsent
  3. Vérifier l'identité MSP utilisée
```

### VPN

**Problème : "Innernet not found"**

```
Cause : Innernet n'est pas installé
Solution :
  # Ubuntu/Debian
  curl -LO https://github.com/tonarino/innernet/releases/latest/download/innernet_amd64.deb
  sudo dpkg -i innernet_amd64.deb

  # Vérification
  innernet --version
```

**Problème : "Connection error: Failed to connect"**

```
Cause : Réseau Innernet non configuré ou coordinateur inaccessible
Solution :
  1. Vérifier que le coordinateur est démarré :
     sudo systemctl status innernet-server@genaptitude

  2. Tester la connectivité :
     ping vpn.genaptitude.local
     nc -zv vpn.genaptitude.local 51820

  3. Vérifier l'invitation :
     sudo innernet show genaptitude

  4. Réinstaller si nécessaire :
     sudo innernet uninstall genaptitude
     sudo innernet install invitation.toml
```

**Problème : "Parse error: Could not parse IP address"**

```
Cause : Format de sortie `innernet show` inattendu
Solution :
  1. Vérifier la version d'Innernet
  2. Utiliser `wg show` en fallback
  3. Vérifier les logs : journalctl -u wg-quick@genaptitude0
```

**Problème : Peers non visibles dans WireGuard**

```
Cause : Handshake WireGuard échoué
Solution :
  1. Vérifier les firewall :
     sudo ufw allow 51820/udp

  2. Vérifier les clés publiques :
     sudo wg show genaptitude0

  3. Forcer un handshake :
     sudo wg set genaptitude0 peer <PUBLIC_KEY> persistent-keepalive 25

  4. Vérifier NAT traversal :
     sudo innernet fetch genaptitude
```

### Général

**Problème : Logs non affichés**

```
Cause : Niveau de log trop élevé
Solution : Ajuster RUST_LOG
  export RUST_LOG=debug
  cargo run
```

**Problème : Permissions insuffisantes**

```
Cause : Certaines opérations nécessitent root
Solution :
  # VPN (nécessite sudo)
  sudo -E cargo run

  # Ou ajouter l'utilisateur au groupe
  sudo usermod -aG sudo $USER
```

---

## 📚 Références

### Documentation Officielle

**Hyperledger Fabric** :

- [Documentation Fabric](https://hyperledger-fabric.readthedocs.io/)
- [Architecture](https://hyperledger-fabric.readthedocs.io/en/latest/architecture.html)
- [MSP](https://hyperledger-fabric.readthedocs.io/en/latest/msp.html)
- [SDK Go](https://github.com/hyperledger/fabric-sdk-go)

**Innernet** :

- [GitHub Innernet](https://github.com/tonarino/innernet)
- [Documentation](https://github.com/tonarino/innernet/blob/main/doc/innernet.8.md)
- [WireGuard](https://www.wireguard.com/)

**Tauri** :

- [Documentation Tauri](https://tauri.app/)
- [State Management](https://tauri.app/v1/guides/features/command#accessing-managed-state)
- [IPC](https://tauri.app/v1/guides/features/command/)

### Dépendances Rust

| Crate                  | Version | Usage                |
| ---------------------- | ------- | -------------------- |
| `tauri`                | 1.x     | Framework applicatif |
| `tokio`                | 1.x     | Runtime async        |
| `serde` / `serde_json` | 1.x     | Sérialisation        |
| `tracing`              | 0.1     | Logging structuré    |
| `thiserror`            | 1.x     | Gestion d'erreurs    |
| `uuid`                 | 1.x     | Génération d'UUIDs   |
| `chrono`               | 0.4     | Gestion du temps     |
| `tonic` (prévu)        | 0.10    | gRPC client          |

### Standards et Protocoles

**Blockchain** :

- [ISO/TC 307](https://www.iso.org/committee/6266604.html) - Blockchain et DLT
- [NIST Blockchain](https://www.nist.gov/blockchain)

**Cryptographie** :

- [RFC 5280](https://tools.ietf.org/html/rfc5280) - X.509
- [RFC 5915](https://tools.ietf.org/html/rfc5915) - ECDSA
- [Noise Protocol](https://noiseprotocol.org/) - WireGuard

**VPN** :

- [RFC 8446](https://tools.ietf.org/html/rfc8446) - TLS 1.3
- [WireGuard Paper](https://www.wireguard.com/papers/wireguard.pdf)

### Documentation Connexe

- `json_db.md` : Module de persistance
- `jsondb_cli_usages.md` : CLI pour la base de données
- `json_db_tests.md` : Suite de tests json_db
- Architecture GenAptitude (à venir)

---

## 📜 Licence

Ce module fait partie de GenAptitude et est soumis à la licence du projet.

---

**Version** : 0.1.0  
**Dernière mise à jour** : Novembre 2024  
**Auteur** : Équipe GenAptitude
