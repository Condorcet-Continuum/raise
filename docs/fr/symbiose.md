# Symbiose & Médiation : Le Contrat Social Numérique

**Statut :** Concept Architecture v1.0
**Projet :** Condorcet Continuum (R.A.I.S.E.)
**Philosophie :** "L'Humain Légifère, la Machine Exécute."

---

## 1. Introduction : La Fin de l'Utilisateur Passif

Dans l'approche Condorcet Continuum, nous rejetons le terme "Utilisateur". Celui qui interagit avec le système n'est pas un consommateur passif d'algorithmes, c'est un **Médiateur Souverain**.

Le système R.A.I.S.E. n'est pas une boîte noire qui décide _à la place_ de l'humain. C'est une administration cognitive ("Glass Box") qui exécute des mandats précis définis par des humains experts.

---

## 2. Les Médiateurs (Le Pouvoir Législatif)

Les humains interviennent en amont (Design) et en aval (Validation). Ils définissent les règles, les valeurs et les limites du système.

### A. Le Métier (Business Owner)

- **Rôle :** Définit la **Finalité**.
- **Mission :** Fixer les objectifs de production, les KPIs cibles et les contraintes opérationnelles.
- **Interaction :** "Je veux maximiser le rendement de la ligne B sans dépasser 2% de rejet."

### B. L'Architecte Numérique (SI, Data, Infra)

- **Rôle :** Définit la **Structure & Faisabilité**.
- **Mission :** Garantir que la donnée est propre (Ontologie), que le code est sûr (Rust/Memory Safe) et que l'infrastructure est frugale (Green IT).
- **Interaction :** "Je définis les schémas JSON stricts et les permissions d'accès aux capteurs."

### C. Le Psychologue & Sociologue (Éthique & Biais)

- **Rôle :** Définit l'**Acceptabilité & Alignement**.
- **Mission :** Ce rôle crucial (souvent oublié) configure les "poids moraux" des agents de vote. Il s'assure que la décision mathématique respecte le contrat social de l'entreprise.
- **Interaction :** "Dans un conflit entre _Vitesse_ et _Sécurité_, l'agent Sécurité doit avoir un poids de vote double."

---

## 3. Les Agents (Le Pouvoir Exécutif)

Le moteur R.A.I.S.E. est composé d'agents spécialisés qui orchestrent les blocs cognitifs. Ils sont autonomes dans leur exécution, mais strictement bornés par le code.

### A. Agents Neuro (Les Traducteurs)

- **Blocs Cognitifs :** Perception, Langage, Attention.
- **Technologie :** LLM Locaux (GGUF), Vision par ordinateur.
- **Fonction :** Ils traduisent l'intention humaine floue ou le signal bruité en une donnée structurée et formelle.
- **Exemple :** "L'opérateur a dit 'Vérifie ça'. Je traduis en commande : `CHECK_STATUS(Target_ID)`."

### B. Agents Symboliques (Les Logiciens)

- **Blocs Cognitifs :** Mémoire, Raisonnement, Apprentissage.
- **Technologie :** Rust, Moteur de Règles, Vector Store.
- **Fonction :** Ils vérifient la cohérence logique, consultent les précédents (RAG) et empêchent les hallucinations.
- **Exemple :** "L'action demandée viole la règle de sécurité #412. Exécution bloquée."

### C. Agents de Consensus (Les Votants)

- **Blocs Cognitifs :** Décision, Résolution.
- **Technologie :** Algorithme de Condorcet.
- **Fonction :** Ce sont des _Personas Virtuels_ qui simulent un débat démocratique interne pour résoudre les conflits d'intérêts.
  - _Agent Sécurité :_ Vote toujours pour la réduction de risque.
  - _Agent Finance :_ Vote toujours pour la réduction de coût.
  - _Agent Ops :_ Vote toujours pour la continuité de service.
- **Résultat :** Le "Vainqueur de Condorcet" est le compromis mathématique optimal entre ces agents.

---

## 4. Le Protocole de Symbiose

Comment ces deux mondes collaborent-ils ? Via une boucle de rétroaction stricte.

1.  **Le Mandat (Humain) :** Les Médiateurs configurent les poids des agents et les règles métier (Fichiers de Config / Ontologie).
2.  **L'Orchestration (Agent) :** Le moteur perçoit, raisonne et vote une proposition d'action (Exécution Neuro-Symbolique).
3.  **La Ratification (Humain) :** L'opérateur final (HITL) valide la proposition.
4.  **L'Ancrage (Système) :** La décision commune est signée cryptographiquement et stockée (Blockchain/Audit Log).

---

> **Note de Synthèse :**
> Dans ce modèle, l'IA ne prend pas le pouvoir. Elle exerce une fonction administrative déléguée, auditable et révocable par les Médiateurs. C'est la garantie de la **Souveraineté Numérique**.
