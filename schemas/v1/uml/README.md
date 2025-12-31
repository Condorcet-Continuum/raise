# 🗺️ Schémas UML (Unified Modeling Language)

Ce répertoire contient l'ensemble des schémas de données **JSON-LD** utilisés par RAISE pour modéliser les systèmes en utilisant la notation **UML (Unified Modeling Language)**.

L'objectif est d'étendre la capacité de la plateforme à l'ingénierie logicielle traditionnelle, en offrant la rigueur du **Model-Based AI Engineering (MBAIE)** à la communauté des architectes UML.

## 🚀 Vision et Objectifs

1.  **Rigueur Formelle :** Garantir que tous les éléments générés par les Agents IA sont valides et respectent la spécification UML 2.5 (validation symbolique).
2.  **Modernisation de l'UML :** Utiliser JSON-LD pour remplacer le format XMI lourd, permettant un **versionnement Git** efficace des architectures logicielles.
3.  **Traçabilité :** Lier les éléments UML au code généré et aux décisions ancrées sur la Blockchain.

## 🌳 Structure de l'Arborescence (`schemas/v1/`)

Afin de situer le contexte UML au sein de la plateforme RAISE, voici l'organisation globale de la version v1 des schémas :

```

schemas/v1/uml/ \<-- Ingénierie Logicielle Standard (Logiciel, IT)
├── **structure/** \<-- Modèles statiques et conceptuels (Ce que le système EST)
│ ├── class-diagram/ \<-- Diagrammes de classes, interfaces, types de données
│ ├── component-diagram/ \<-- Organisation des composants
│ └── composite-structure-diagram/ \<-- Structure interne des classes et composants
│
├── **behavioral/** \<-- Modèles dynamiques et fonctionnels (Ce que le système FAIT)
│ ├── use-case-diagram/ \<-- Cas d'utilisation
│ ├── activity-diagram/ \<-- Flux d'activités (Workflows)
│ └── state-machine-diagram/ \<-- États et transitions (Comportement réactif)
│
├── **interaction/** \<-- Modèles de communication (Comment le système INTERAGIT)
│ ├── sequence-diagram/ \<-- Séquences d'appels entre objets
│ └── communication-diagram/ \<-- (Ancien diagramme de collaboration)
│
├── **deployment/** \<-- Modèles physiques (Où le système TOURNE)
│ ├── deployment-diagram/ \<-- Nœuds matériels et déploiement des artefacts
│ └── profile-diagram/ \<-- Définition des stéréotypes et extensions
│
└── **common/** \<-- Éléments partagés et fondamentaux
├── element-base.jsonld \<-- Schéma de base pour tout élément UML (ID, nom, description, stéréotypes)
├── relationship-base.jsonld \<-- Schéma pour les relations (Association, Dépendance, etc.)
└── data-types/ \<-- Types de données primitives (String, Integer, etc.)

```

## ✍️ Guide de Contribution

Tout nouveau schéma UML doit respecter les principes suivants pour être intégré au `Model Engine` :

1.  **Format JSON-LD Stricte :** Les fichiers doivent être au format `.jsonld` pour garantir une sémantique cohérente et une indexation efficace.
2.  **Réutilisation des Bases :** Chaque schéma doit étendre le schéma de base approprié dans `common/` (ex: `element-base.jsonld`) pour assurer une cohérence des champs fondamentaux (ID, Nom, Description).
3.  **Validation :** Après l'ajout ou la modification d'un schéma, la cohérence doit être vérifiée via les tests du `Model Engine`.

```

```
