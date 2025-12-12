// FICHIER : src/types/arcadia.types.ts

export const Namespaces = {
  ARCADIA: 'https://genaptitude.io/ontology/arcadia#',
  OA: 'https://genaptitude.io/ontology/arcadia/oa#',
  SA: 'https://genaptitude.io/ontology/arcadia/sa#',
  LA: 'https://genaptitude.io/ontology/arcadia/la#',
  PA: 'https://genaptitude.io/ontology/arcadia/pa#',
  EPBS: 'https://genaptitude.io/ontology/arcadia/epbs#',
  DATA: 'https://genaptitude.io/ontology/arcadia/data#',
} as const;

export const ArcadiaTypes = {
  // OA
  OA_ACTOR: `${Namespaces.OA}OperationalActor`,
  OA_ACTIVITY: `${Namespaces.OA}OperationalActivity`,
  OA_CAPABILITY: `${Namespaces.OA}OperationalCapability`,

  // SA
  SA_FUNCTION: `${Namespaces.SA}SystemFunction`,
  SA_COMPONENT: `${Namespaces.SA}SystemComponent`,
  SA_ACTOR: `${Namespaces.SA}SystemActor`,

  // LA
  LA_COMPONENT: `${Namespaces.LA}LogicalComponent`,
  LA_FUNCTION: `${Namespaces.LA}LogicalFunction`,

  // PA
  PA_COMPONENT: `${Namespaces.PA}PhysicalComponent`,

  // DATA
  DATA_CLASS: `${Namespaces.DATA}Class`,
  DATA_TYPE: `${Namespaces.DATA}DataType`,
} as const;

// Helper Type Guard
export function isArcadiaType(elementKind: string | undefined, targetType: string): boolean {
  return elementKind === targetType;
}
