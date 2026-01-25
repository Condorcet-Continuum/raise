# Protocoles IA : Architecture Neuro-Symbolique (A2A & MCP)

Ce module implémente les standards de communication qui structurent l'intelligence de Raise.
Il concrétise l'approche **Neuro-Symbolique** en séparant strictement la _négociation_ (l'intention) de l' _exécution_ (l'action).

## 1. Philosophie : Le Cerveau Bicaméral

Pour éviter les hallucinations et garantir la sécurité industrielle (DO-178C), Raise n'utilise pas un LLM monolithique, mais une chaîne de commande :

1.  **Couche Décisionnelle (A2A) :** Un "Conseil d'Administration" où des agents neuronaux (créatifs) et symboliques (règles strictes) débattent.
2.  **Couche Exécutive (MCP) :** Des "Bras" techniques qui exécutent les ordres validés sans ambiguïté.

### Comparatif des Protocoles

| Critère              | **A2A (FIPA ACL)**                                      | **MCP (Model Context Protocol)**                             |
| :------------------- | :------------------------------------------------------ | :----------------------------------------------------------- |
| **Rôle**             | **Coordination Sociale** : Négocier, demander, refuser. | **Connexion Technique** : Lire un fichier, requêter une API. |
| **Philosophie**      | "Actes de Langage" (L'intention prime).                 | "Plug-and-Play" (L'accès prime).                             |
| **Abstraction**      | **Haut** : "Je _veux_ que tu fasses X".                 | **Bas** : "Exécute la fonction Y".                           |
| **Gestion d'Erreur** | Sociale (Refus, Contre-proposition).                    | Technique (Exception, Timeout).                              |
| **Analogie**         | Le **Langage** (Diplomatie).                            | La **Main** (Outil).                                         |

---

## 2. Architecture en Couches : Traçabilité et Rôles

Cette architecture sépare strictement la prise de décision ("Pourquoi on le fait") de l'exécution technique ("Comment on le fait").

```mermaid
graph TD
    subgraph "Couche Décisionnelle (A2A)"
        A[Agent Orchestrateur LLM] <-->|FIPA ACL: Négociation| B[Agent Expert Symbolique]
        A <-->|FIPA ACL: Coordination| C[Agent Auditeur]
    end

    subgraph "Couche Interface"
        A -->|Validation| D[Client MCP]
    end

    subgraph "Couche Exécution (MCP)"
        D -->|JSON-RPC| E[Serveur MCP: Filesystem]
        D -->|JSON-RPC| F[Serveur MCP: Database]
        D -->|JSON-RPC| G[Serveur MCP: GitHub]
    end

    style B fill:#f9f,stroke:#333,stroke-width:2px,color:black
    style E fill:#bbf,stroke:#333,stroke-width:2px,color:black
    style F fill:#bbf,stroke:#333,stroke-width:2px,color:black
    style G fill:#bbf,stroke:#333,stroke-width:2px,color:black

```

### Bénéfices pour la Sécurité :

- **Isolation :** Les agents LLM (Couche Décisionnelle) n'ont _aucun_ accès direct aux Serveurs MCP. Ils doivent passer par le Client MCP, qui n'obéit qu'aux ordres validés.
- **Audit :** L'Agent Auditeur (C) enregistre les décisions avant qu'elles ne deviennent des actions.

---

## 3. Le Flux de Contrôle Sécurisé

Le but principal est d'empêcher un LLM d'utiliser directement un outil critique. Il doit passer par une "Soupape de Sécurité" logique.

### Diagramme de Séquence (Validation)

```mermaid
sequenceDiagram
    participant User as Utilisateur
    participant LLM as Agent Neuronal (LLM)
    participant Sym as Agent Symbolique (Logique)
    participant MCP as Serveur MCP (Outils)

    User->>LLM: "Supprime les logs pour faire de la place"

    Note over LLM: Analyse probabiliste

    LLM->>Sym: ACL PROPOSE (Action: Delete, Target: Logs)

    Note over Sym: Vérification Règles (If Date=Today Then DENY)

    alt Règle Violée (Sécurité)
        Sym-->>LLM: ACL REFUSE (Raison: Violation Règle #42)
        LLM->>User: "Je ne peux pas : le règlement l'interdit."
    else Règle Validée
        Sym-->>LLM: ACL AGREE
        LLM->>MCP: MCP Request: call_tool("delete_logs")
        MCP-->>LLM: Confirm "Logs deleted"
        LLM->>User: "C'est fait."
    end

```

---

## 4. Détails Techniques

### Protocol A2A (Agent-to-Agent)

Défini dans `acl.rs`. Utilise des **Performatifs** pour typer l'échange :

- `REQUEST` : Demande d'action.
- `PROPOSE` : Proposition de solution (souvent par le LLM).
- `REFUSE` : Le "Non" logique (émis par le Moteur de Règles).
- `CONFIRM` : Le feu vert pour passer à l'étape MCP.

### Protocol MCP (Model Context Protocol)

Défini dans `mcp.rs`. Standardise l'appel d'outil via JSON-RPC :

- `ToolCall` : Nom de l'outil + Arguments JSON.
- `ToolResult` : Succès ou Erreur technique.

```


```
