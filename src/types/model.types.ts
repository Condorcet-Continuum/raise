// FICHIER : src/types/model.types.ts

export interface I18nString {
  [lang: string]: string;
}

export interface ArcadiaElement {
  id: string;
  name: string | I18nString;
  type?: string; // URI complète (ex: https://...#OperationalActor)
  description?: string;
  // Propriétés dynamiques (PVMT, attributs métier)
  [key: string]: any;
}

// --- Couches (Layers) ---

export interface OperationalAnalysisLayer {
  actors: ArcadiaElement[];
  activities: ArcadiaElement[];
  capabilities: ArcadiaElement[];
  entities: ArcadiaElement[];
  exchanges: ArcadiaElement[];
}

export interface SystemAnalysisLayer {
  components: ArcadiaElement[];
  actors: ArcadiaElement[];
  functions: ArcadiaElement[];
  capabilities: ArcadiaElement[];
  exchanges: ArcadiaElement[];
}

export interface LogicalArchitectureLayer {
  components: ArcadiaElement[];
  actors: ArcadiaElement[];
  functions: ArcadiaElement[];
  interfaces: ArcadiaElement[];
  exchanges: ArcadiaElement[];
}

export interface PhysicalArchitectureLayer {
  components: ArcadiaElement[];
  actors: ArcadiaElement[];
  functions: ArcadiaElement[];
  links: ArcadiaElement[];
  exchanges: ArcadiaElement[];
}

export interface EPBSLayer {
  configurationItems: ArcadiaElement[];
}

export interface DataLayer {
  classes: ArcadiaElement[];
  dataTypes: ArcadiaElement[];
  exchangeItems: ArcadiaElement[];
}

// --- Méta-données ---

export interface ProjectMeta {
  name?: string;
  version?: string;
  loadedAt?: string;
  elementCount?: number;
  description?: string;
}

// --- Racine du Projet ---

export interface ProjectModel {
  id: string;
  name?: string; // Nom à la racine (souvent redondant avec meta.name)

  // Les couches sont optionnelles car un projet peut être partiel
  oa?: OperationalAnalysisLayer;
  sa?: SystemAnalysisLayer;
  la?: LogicalArchitectureLayer;
  pa?: PhysicalArchitectureLayer;
  epbs?: EPBSLayer;
  data?: DataLayer;

  meta?: ProjectMeta;

  // Flexibilité pour extensions futures
  [key: string]: any;
}
