import { Canvas } from '@react-three/fiber';
import { OrbitControls, Stars, Text } from '@react-three/drei';
import { useEffect, useState, useMemo } from 'react';
import { useUiStore } from '../../store/ui-store';
import { spatialService } from '../../services/spatial-service';
import * as THREE from 'three';

// 1. Définition des types pour éviter le 'any'
interface SpatialNode {
  id: string;
  label: string;
  position: [number, number, number];
  layer?: number;
  weight?: number;
}

interface SpatialLink {
  source: string;
  target: string;
  value?: number;
}

interface TopologyData {
  nodes: SpatialNode[];
  links: SpatialLink[];
}

// --- Composant Interne : Gestion des Nœuds ---
function GraphNodes() {
  // 2. Utilisation du type TopologyData
  const [data, setData] = useState<TopologyData>({ nodes: [], links: [] });
  const setSelection = useUiStore((s) => s.setSelection);

  useEffect(() => {
    // Le service doit renvoyer des données compatibles avec notre interface
    spatialService.getTopology().then((topology) => {
      // On force le typage ici si le service retourne 'any', ou on laisse l'inférence
      setData(topology as unknown as TopologyData);
    });
  }, []);

  // Géométrie et Matériau partagés
  const nodeGeometry = useMemo(() => new THREE.SphereGeometry(1, 32, 32), []);
  const nodeMaterial = useMemo(
    () => new THREE.MeshStandardMaterial({ color: '#4f46e5', roughness: 0.3 }),
    [],
  );

  return (
    <group>
      {data.nodes.map((node) => (
        <mesh
          key={node.id}
          position={node.position}
          onClick={(e) => {
            e.stopPropagation();
            setSelection(node.id, 'spatial');
          }}
          geometry={nodeGeometry}
          material={nodeMaterial}
        >
          <Text
            position={[0, 1.5, 0]}
            fontSize={0.5}
            color="white"
            anchorX="center"
            anchorY="middle"
          >
            {node.label}
          </Text>
        </mesh>
      ))}

      {/* Liens (Lignes simples) */}
      {data.links.map((link, i) => {
        const source = data.nodes.find((n) => n.id === link.source);
        const target = data.nodes.find((n) => n.id === link.target);

        // Si un nœud manque, on ne dessine pas le lien
        if (!source || !target) return null;

        // 3. Correction : On utilise directement les positions pour la géométrie
        // La variable inutile 'points' a été supprimée.
        return (
          <line key={i}>
            <bufferGeometry>
              <bufferAttribute
                attach="attributes-position"
                count={2}
                array={new Float32Array([...source.position, ...target.position])}
                itemSize={3}
              />
            </bufferGeometry>
            <lineBasicMaterial attach="material" color="#94a3b8" transparent opacity={0.5} />
          </line>
        );
      })}
    </group>
  );
}

// --- Composant Principal ---
export function SpatialScene() {
  const setSelection = useUiStore((s) => s.setSelection);
  const [isReady, setIsReady] = useState(false);

  useEffect(() => {
    const timer = setTimeout(() => setIsReady(true), 100);
    return () => clearTimeout(timer);
  }, []);

  if (!isReady) {
    return (
      <div
        style={{
          width: '100%',
          height: '100%',
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'center',
          color: '#64748b',
        }}
      >
        INITIALIZING SPATIAL ENGINE...
      </div>
    );
  }

  return (
    <div style={{ width: '100%', height: '100%', backgroundColor: '#000000' }}>
      <Canvas
        camera={{ position: [0, 5, 10], fov: 60 }}
        style={{ width: '100%', height: '100%', display: 'block' }}
        onPointerMissed={() => setSelection(null)}
      >
        <color attach="background" args={['#0f172a']} />

        <ambientLight intensity={0.5} />
        <pointLight position={[10, 10, 10]} intensity={1} />

        <Stars radius={100} depth={50} count={5000} factor={4} saturation={0} fade speed={1} />

        <GraphNodes />

        <OrbitControls makeDefault />
      </Canvas>
    </div>
  );
}
