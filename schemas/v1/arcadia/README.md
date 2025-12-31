# Modèle de Données Arcadia (RAISE)

Ce répertoire contient la définition formelle du méta-modèle **Arcadia** (Architecture Analysis & Design Integrated Approach) adapté pour la plateforme **RAISE**.

Il repose sur une architecture **MBAIE** (Model-Based AI Engineering) combinant :

1. **JSON Schema (Draft 2020-12)** : Pour la validation structurelle stricte.
2. **JSON-LD** : Pour la sémantique, le typage et le graphe de connaissances.
3. **Moteur `x_compute`** : Pour le calcul automatique des métadonnées techniques.

---

## 1. Architecture d'Héritage

Tous les schémas métiers héritent d'un socle commun pour garantir l'uniformité technique et fonctionnelle.

```mermaid
classDiagram
    class PrimitiveTypes {
        UUID, URI, Date
        i18nString
        x_compute rules
    }
    class BaseSchema {
        $schema
        id (UUID)
        createdAt
        updatedAt
    }
    class Metamodel {
        xmi_id
        name (i18n)
        description (i18n)
        summary (i18n)
        tags
        propertyValues (PVMT)
    }
    class BusinessObject {
        Champs spécifiques
        (ex: isHuman, flowType)
    }

    BaseSchema --|> PrimitiveTypes : Utilise
    Metamodel --|> PrimitiveTypes : Utilise
    BusinessObject --|> BaseSchema : Hérite (allOf)
    BusinessObject --|> Metamodel : Hérite (allOf)
```

### Hiérarchie des Schémas

- **`common/types/primitive-types`** : Définit les formats (UUID, URI) et les règles de calcul (`x_compute`).
- **`common/base.schema`** : Assure que chaque objet a une identité unique et des timestamps.
- **`metamodel/metamodel.schema`** : Fournit les propriétés communes à tous les objets Arcadia (Nom, Description multilingue, Lien XMI, Extensions PVMT).

---

## 2. Couches d'Ingénierie (Layers)

L'architecture respecte les 5 niveaux d'abstraction de la méthode Arcadia.

### 🟢 OA - Operational Analysis (Analyse Opérationnelle)

Définition du problème et du besoin utilisateur (Quoi et Pourquoi).

| Schéma                    | Description                                                               |
| ------------------------- | ------------------------------------------------------------------------- |
| **OperationalActor**      | Entité (humaine ou non) interagissant avec l'organisation.                |
| **OperationalEntity**     | Organisation, service ou groupe d'acteurs.                                |
| **OperationalActivity**   | Tâche ou action métier effectuée par un acteur/entité.                    |
| **OperationalCapability** | Aptitude de l'organisation à fournir un service (regroupe des activités). |
| **OperationalExchange**   | Flux d'information ou matière entre activités/acteurs.                    |
| **OperationalRole**       | Ensemble de responsabilités endossé par un acteur.                        |

### 🟡 SA - System Analysis (Analyse Système)

Définition du système comme une boîte noire (Ce que le système doit faire).

| Schéma                 | Description                                              |
| ---------------------- | -------------------------------------------------------- |
| **SystemComponent**    | Le système lui-même (Frontière).                         |
| **SystemActor**        | Acteur externe interagissant avec le système.            |
| **SystemFunction**     | Fonctionnalité attendue du système.                      |
| **SystemCapability**   | Capacité du système traçant vers un besoin opérationnel. |
| **FunctionalExchange** | Flux de données entre fonctions système.                 |

### 🔵 LA - Logical Architecture (Architecture Logique)

Définition de la solution (Comment le système fonctionne, boîte blanche).

| Schéma                 | Description                                       |
| ---------------------- | ------------------------------------------------- |
| **LogicalComponent**   | Brique structurelle du système (non-physique).    |
| **LogicalFunction**    | Raffinement d'une fonction système.               |
| **LogicalActor**       | Acteur logique interagissant avec le système.     |
| **LogicalInterface**   | Contrat d'échange (API, Protocole).               |
| **ComponentExchange**  | Connexion logique entre deux composants.          |
| **FunctionalExchange** | Flux de données raffiné entre fonctions logiques. |

### 🔴 PA - Physical Architecture (Architecture Physique)

Implémentation concrète (Matériel, Logiciel, Déploiement).

| Schéma                | Description                                                     |
| --------------------- | --------------------------------------------------------------- |
| **PhysicalComponent** | Node (Matériel) ou Behavior (Logiciel). Gère le déploiement.    |
| **PhysicalLink**      | Liaison physique (Câble, Bus, Réseau, Ondes).                   |
| **PhysicalFunction**  | Fonction terminale ("Feuille") exécutée par un composant.       |
| **PhysicalActor**     | Acteur physique.                                                |
| **ComponentExchange** | Connexion logicielle/physique transportée par un lien physique. |

### 🟣 EPBS - End Product Breakdown Structure

Décomposition industrielle et configuration.

| Schéma                | Description                                                                    |
| --------------------- | ------------------------------------------------------------------------------ |
| **ConfigurationItem** | Élément livrable (HWCI, CSCI, SystemPart) regroupant des composants physiques. |

---

## 3. Modèle de Données (Data)

Définit la structure des informations échangées par les fonctions.

- **DataType** : Types primitifs (Integer, Float, Boolean) et Énumérations.
- **DataClass** : Structures complexes composées d'attributs (champs) typés. Supporte l'héritage.
- **ExchangeItem** : Le contrat d'échange (paquet de données) qui circule sur les flux fonctionnels.

---

## 4. Concepts Transverses & IVVQ

Éléments applicables à toutes les couches pour la spécification, la vérification et la qualité.

### Ingénierie des Exigences

- **Requirement** : Exigence formelle (ID, texte, justification).
- **Constraint** : Restriction technique ou physique (Expression formelle).

### Comportement Dynamique

- **FunctionalChain** : Séquence ordonnée de fonctions/échanges (Chemin critique).
- **ExchangeScenario** : Diagramme de séquence (Interactions temporelles).
- **StateMachine** : Automate décrivant les Modes et États.

### IVVQ (Intégration, Vérification, Validation, Qualité)

- **TestProcedure** : Protocole de test (pas à pas).
- **TestExecution** : Résultat d'un run de test (Preuves, Verdict).
- **TestCampaign** : Regroupement de tests pour une version.
- **QualityRule** : Règle de validation du modèle (ex: complexité cyclomatique).
- **QualityAssessment** : Rapport d'audit qualité sur un élément.

### PVMT (Property Values Management Tool)

Mécanisme d'extension pour ajouter des données métiers (Masse, Coût, Puissance...).

- **PropertyDefinition** : Le modèle de la propriété (Nom, Type, Unité).
- **PropertyValue** : La valeur instanciée sur un objet.

---

## 5. Sémantique (JSON-LD)

Le répertoire `@context` contient les définitions sémantiques permettant de transformer les fichiers JSON en graphe de connaissances RDF.

- **`arcadia.jsonld`** : Contexte racine, importe les sous-contextes.
- **`oa.jsonld`, `sa.jsonld`, ...** : Vocabulaire spécifique à chaque couche.

### Relations clés

- **`realizes` / `realizedBy`** : Traçabilité verticale (ex: SA vers OA).
- **`satisfiedBy` / `verifiedBy`** : Traçabilité des exigences et tests.
- **`allocatedTo`** : Lien Fonction → Composant.
- **`propertyValues`** : Lien vers les extensions PVMT.

---

## Exemple d'Instance (JSON)

Voici à quoi ressemble un objet **System Function** complet dans la base :

```json
{
  "$schema": "../../schemas/v1/arcadia/sa/system-function.schema.json",
  "@context": "https://raise.io/ontology/arcadia/arcadia.jsonld",
  "id": "urn:uuid:c5e8f9a0-58cc-4372-a567-0e02b2c3d479",
  "xmi_id": "_18_0_2_4a901be_163549382_446954_4713",
  "name": {
    "fr": "Analyser Flux Vidéo",
    "en": "Analyze Video Stream"
  },
  "description": {
    "en": "Detects obstacles in real-time."
  },
  "inputs": ["urn:uuid:exchange-video-raw"],
  "outputs": ["urn:uuid:exchange-obstacle-data"],
  "realizedActivities": ["urn:uuid:act-surveiller-zone"],
  "propertyValues": [
    {
      "definitionId": "prop-performance",
      "values": { "latency_ms": 20 }
    }
  ],
  "createdAt": "2025-11-27T10:00:00Z",
  "updatedAt": "2025-11-27T10:05:00Z"
}
```

---

## Résumé

Ce méta-modèle Arcadia pour RAISE offre :

- ✅ **Validation stricte** via JSON Schema
- ✅ **Sémantique enrichie** via JSON-LD et ontologies
- ✅ **Traçabilité multi-niveaux** entre couches d'ingénierie
- ✅ **Extensibilité** via PVMT
- ✅ **IVVQ intégré** pour la qualité et la vérification

Il constitue la fondation du système d'ingénierie dirigé par les modèles et augmenté par l'IA de RAISE.
