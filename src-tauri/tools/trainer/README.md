# 🎓 Module d'Entraînement IA (GenAptitude Trainer)

Ce module permet d'effectuer le **Fine-Tuning** (raffinage) de modèles de langage (LLM) à partir des données exportées par GenAptitude.

Il utilise la technique **QLoRA** (Quantized Low-Rank Adaptation) pour adapter efficacement un modèle générique (ex: Qwen 2.5) à votre domaine spécifique (Ingénierie Système, Arcadia, etc.).

## 📂 Structure du Dossier

```text
tools/trainer/
├── dataset.jsonl       # Données d'entraînement (généré par le backend Rust)
├── train.py            # Script principal d'entraînement (PyTorch/Peft/TRL)
├── requirements.txt    # Liste des dépendances Python
└── venv/               # Environnement virtuel (Local uniquement - Ignoré par Git)

```

## 🛠️ Prérequis

Avant de lancer un entraînement, vous devez générer le fichier de données `dataset.jsonl`.

- **Via la CLI Rust :** `cargo run --bin genaptitude ai_export_dataset`
- **Via l'Application :** En utilisant la commande d'export dans la Console Développeur.

---

## 🚀 Option A : Entraînement Local (Linux / WSL)

Utilisez cette méthode si vous possédez une machine équipée d'un **GPU NVIDIA performant** (RTX 3060/4060 ou supérieur avec 8Go+ de VRAM).

### 1. Préparer l'environnement

Ne lancez pas ces commandes en tant que root. Créez un environnement isolé :

```bash
cd src-tauri/tools/trainer

# Création de l'environnement virtuel
python3 -m venv venv

# Activation
source venv/bin/activate

# Installation des librairies
pip install -r requirements.txt

```

### 2. Lancer l'entraînement

Assurez-vous que le fichier `dataset.jsonl` est présent dans le dossier.

```bash
python train.py

```

_Note : Si vous rencontrez des erreurs de mémoire (OOM), réduisez le paramètre `per_device_train_batch_size` dans `train.py`._

---

## ☁️ Option B : Google Colab (Gratuit / GPU T4)

Utilisez cette méthode si vous n'avez pas de GPU dédié ou si vous avez un GPU ancien (ex: GTX 9xx) incompatible avec les formats modernes.

**Le script `train.py` a été optimisé pour les GPU Tesla T4 (offre gratuite Colab) en forçant le calcul en FP32 pour éviter les erreurs BFloat16.**

### 1. Initialiser Google Colab

1. Rendez-vous sur [Google Colab](https://colab.research.google.com/).
2. Créez un **Nouveau Notebook**.
3. Allez dans le menu **Exécution** > **Modifier le type d'exécution**.
4. Sélectionnez **T4 GPU** et validez.

### 2. Importer les fichiers

Dans le volet de gauche (icône Dossier 📁), glissez-déposez les 3 fichiers suivants depuis votre dossier local `tools/trainer` :

- `train.py`
- `requirements.txt`
- `dataset.jsonl`

### 3. Installer les dépendances

Créez une cellule de code et exécutez :

```python
!pip install -r requirements.txt

```

### 4. Lancer l'entraînement

Créez une deuxième cellule et exécutez :

```python
!python train.py

```

### 5. Récupérer le modèle ("Cerveau")

Colab ne permet pas de télécharger un dossier directement. Compressez le résultat :

```bash
!zip -r mon_modele.zip genaptitude-qwen-adapter

```

Ensuite, faites un clic droit sur `mon_modele.zip` dans le volet de fichiers et choisissez **Télécharger**.

---

## ⚙️ Détails Techniques

### Configuration du Script (`train.py`)

Le script est configuré pour être robuste face aux limitations matérielles :

- **Modèle Cible :** `Qwen/Qwen2.5-1.5B-Instruct` (Léger et performant).
- **Quantization :** 4-bit (NF4) via `bitsandbytes`.
- **Mode Compatibilité T4 :**
- `bnb_4bit_compute_dtype = torch.float32` : Force les calculs en précision standard (évite les bugs sur architecture Turing/Pascal).
- `fp16 = False` & `bf16 = False` : Désactive la précision mixte pour garantir la stabilité.

- **WandB :** Désactivé par défaut (`os.environ["WANDB_DISABLED"] = "true"`) pour éviter les interruptions.

### Résultat (Output)

L'entraînement génère un adaptateur LoRA composé de deux fichiers principaux :

- `adapter_config.json` : Les hyperparamètres du réseau.
- `adapter_model.safetensors` : Les poids entraînés (environ 50-200 Mo).

## 📥 Intégration dans GenAptitude

Pour utiliser votre modèle entraîné :

1. Créez le dossier de stockage :

```bash
mkdir -p ~/genaptitude-llm/ai-assets/lora

```

2. Décompressez votre modèle à l'intérieur.
3. Configurez votre fichier `.env` (si supporté par la version actuelle) :

```ini
RAISE_LORA_PATH=~/genaptitude-llm/ai-assets/lora/genaptitude-qwen-adapter

```

```


```
