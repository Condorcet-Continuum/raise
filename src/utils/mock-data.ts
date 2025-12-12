import type { ProjectModel } from '@/types/model.types';

export const MOCK_PROJECT: ProjectModel = {
  id: 'proj-demo-1',
  name: 'Drone Surveillance System',
  meta: {
    name: 'Drone Surveillance System',
    version: '2.4.0',
    loadedAt: new Date().toISOString(),
    elementCount: 42,
    description:
      'Système de drone autonome pour la surveillance maritime et la détection de pollutions.',
  },

  // 1. Analyse Opérationnelle (OA)
  oa: {
    actors: [
      { id: 'oa-1', name: 'Opérateur Maritime', type: 'OperationalActor' },
      { id: 'oa-2', name: 'Autorité Portuaire', type: 'OperationalActor' },
    ],
    activities: [
      { id: 'oa-act-1', name: 'Surveiller Zone Côtière', type: 'OperationalActivity' },
      { id: 'oa-act-2', name: 'Signaler Incident', type: 'OperationalActivity' },
    ],
    capabilities: [],
    entities: [],
    exchanges: [],
  },

  // 2. Analyse Système (SA)
  sa: {
    components: [{ id: 'sa-1', name: 'Système Drone', type: 'SystemComponent' }],
    functions: [
      { id: 'sa-fn-1', name: 'Voler Automatiquement', type: 'SystemFunction' },
      { id: 'sa-fn-2', name: 'Capturer Vidéo', type: 'SystemFunction' },
    ],
    actors: [],
    capabilities: [],
    exchanges: [],
  },

  // 3. Architecture Logique (LA)
  la: {
    components: [
      {
        id: 'lc-1',
        name: 'Flight Controller',
        type: 'LogicalComponent',
        description: 'Gère la stabilisation et la navigation.',
      },
      {
        id: 'lc-2',
        name: 'Camera Module',
        type: 'LogicalComponent',
        description: 'Capture flux vidéo 4K.',
      },
      {
        id: 'lc-3',
        name: 'Telemetry Link',
        type: 'LogicalComponent',
        description: 'Transmission radio longue portée.',
      },
      {
        id: 'lc-4',
        name: 'Power Manager',
        type: 'LogicalComponent',
        description: 'Distribution énergie batterie.',
      },
    ],
    functions: [
      { id: 'lf-1', name: 'Process Video Stream', type: 'LogicalFunction' },
      { id: 'lf-2', name: 'Compute Trajectory', type: 'LogicalFunction' },
      { id: 'lf-3', name: 'Encrypt Data', type: 'LogicalFunction' },
    ],
    interfaces: [{ id: 'li-1', name: 'Video Interface', type: 'Interface' }],
    actors: [],
    exchanges: [],
  },

  // 4. Architecture Physique (PA)
  pa: {
    components: [
      {
        id: 'pc-1',
        name: 'Nvidia Jetson Orin',
        type: 'PhysicalComponent',
        description: 'Calculateur embarqué IA.',
      },
      {
        id: 'pc-2',
        name: 'Sony IMX Sensor',
        type: 'PhysicalComponent',
        description: 'Capteur optique.',
      },
      { id: 'pc-3', name: 'LiPo Battery 6S', type: 'PhysicalComponent' },
    ],
    actors: [],
    functions: [],
    links: [],
    exchanges: [],
  },

  // 5. EPBS (Produit)
  epbs: {
    configurationItems: [
      { id: 'ci-1', name: 'Drone Chassis HW', type: 'ConfigurationItem' },
      { id: 'ci-2', name: 'Flight Software V2', type: 'ConfigurationItem' },
    ],
  },

  // 6. Données
  data: {
    classes: [
      { id: 'cls-1', name: 'VideoFrame', type: 'Class' },
      { id: 'cls-2', name: 'GPSCoordinate', type: 'Class' },
      { id: 'cls-3', name: 'TelemetryPacket', type: 'Class' },
    ],
    dataTypes: [],
    exchangeItems: [],
  },
};
