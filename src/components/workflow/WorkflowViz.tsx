// FICHIER : src/components/workflow/WorkflowViz.tsx

import { useEffect } from 'react';
import ReactFlow, {
  Background,
  Controls,
  Node,
  Edge,
  useNodesState,
  useEdgesState,
} from 'reactflow';
import 'reactflow/dist/style.css';

interface WorkflowVizProps {
  logs: string[];
  globalStatus: string;
}

// --- CONFIGURATION DU GRAPHE (Mise à jour avec le nœud WASM) ---
const INITIAL_NODES: Node[] = [
  {
    id: 'start',
    position: { x: 250, y: 0 },
    data: { label: '🚀 START\n(Mandat)' },
    type: 'input',
    style: { background: '#fff', border: '1px solid #777', width: 150 },
  },
  {
    id: 'tool_read',
    position: { x: 250, y: 80 },
    data: { label: '🔌 SENSOR\n(Lecture MCP)' },
    style: { background: '#fff', border: '1px solid #777', width: 150 },
  },
  {
    id: 'gate_veto',
    position: { x: 250, y: 160 },
    data: { label: '🛡️ VETO HARD\n(Règle Statique)' },
    style: { background: '#fff', border: '1px solid #777', width: 150 },
  },
  // --- NOUVEAU NŒUD WASM ---
  {
    id: 'wasm_policy',
    position: { x: 250, y: 240 },
    data: { label: '🔮 WASM\n(Hot-Swap Policy)' },
    style: { background: '#f0fdf4', border: '2px dashed #16a34a', width: 150 },
  },
  // -------------------------
  {
    id: 'agent_exec',
    position: { x: 250, y: 320 },
    data: { label: '🤖 AGENT IA\n(Stratégie)' },
    style: { background: '#fff', border: '1px solid #777', width: 150 },
  },
  {
    id: 'vote',
    position: { x: 250, y: 400 },
    data: { label: '🗳️ CONDORCET\n(Consensus)' },
    style: { background: '#fff', border: '1px solid #777', width: 150 },
  },
  {
    id: 'end',
    position: { x: 250, y: 480 },
    data: { label: '🏁 END\n(Mission Accomplie)' },
    type: 'output',
    style: { background: '#fff', border: '1px solid #777', width: 150 },
  },
];

const INITIAL_EDGES: Edge[] = [
  { id: 'e1-2', source: 'start', target: 'tool_read', animated: true },
  { id: 'e2-3', source: 'tool_read', target: 'gate_veto', animated: true },
  { id: 'e3-wasm', source: 'gate_veto', target: 'wasm_policy', animated: true }, // Vers WASM
  { id: 'ewasm-4', source: 'wasm_policy', target: 'agent_exec', animated: true }, // Depuis WASM
  { id: 'e4-5', source: 'agent_exec', target: 'vote', animated: true },
  { id: 'e5-6', source: 'vote', target: 'end', animated: true },
];

export default function WorkflowViz({ logs, globalStatus }: WorkflowVizProps) {
  const [nodes, setNodes, onNodesChange] = useNodesState(INITIAL_NODES);
  const [edges, setEdges, onEdgesChange] = useEdgesState(INITIAL_EDGES);

  useEffect(() => {
    const logString = logs.join('\n');

    setNodes((currentNodes) =>
      currentNodes.map((node) => {
        const style = { ...node.style, transition: 'all 0.5s ease' };

        let isActive = false;
        let isError = false;

        // --- Logique de détection basée sur les logs Backend ---
        if (node.id === 'start' && logString.includes('Initialisation Mandat')) isActive = true;
        if (node.id === 'tool_read' && logString.includes('Lecture Capteur')) isActive = true;

        // Veto Hard
        if (
          node.id === 'gate_veto' &&
          (logString.includes('Vérification Veto') || logString.includes('VETO'))
        ) {
          isActive = true;
          if (logString.includes('VETO DÉCLENCHÉ')) isError = true;
        }

        // WASM (Nouveau)
        if (
          node.id === 'wasm_policy' &&
          (logString.includes('WASM') || logString.includes('Gouvernance Dynamique'))
        ) {
          isActive = true;
          if (logString.includes('WASM VETO') || logString.includes('Refus WASM')) isError = true;
        }

        if (node.id === 'agent_exec' && logString.includes('Exécution Stratégie')) isActive = true;
        if (node.id === 'vote' && logString.includes('Vote Condorcet')) isActive = true;
        if (node.id === 'end' && logString.includes('Fin de Mission')) isActive = true; // "Fin de Mission" correspond au compiler

        // Application styles
        if (isError) {
          style.background = '#fee2e2';
          style.borderColor = '#ef4444';
          style.boxShadow = '0 0 15px rgba(239, 68, 68, 0.6)';
          style.color = '#7f1d1d';
        } else if (isActive) {
          style.background = '#dcfce7';
          style.borderColor = '#22c55e';
          style.boxShadow = '0 0 10px rgba(34, 197, 94, 0.4)';
          style.color = '#14532d';
        } else {
          style.background = '#1e293b';
          style.borderColor = '#475569';
          style.color = '#94a3b8';
        }

        return { ...node, style };
      }),
    );

    setEdges((currentEdges) =>
      currentEdges.map((edge) => ({
        ...edge,
        animated: globalStatus === 'Running' || globalStatus === 'Pending',
        style: {
          stroke:
            globalStatus === 'Failed'
              ? '#ef4444'
              : globalStatus === 'Completed'
              ? '#22c55e'
              : '#64748b',
          strokeWidth: 2,
        },
      })),
    );
  }, [logs, globalStatus, setNodes, setEdges]);

  return (
    <div style={{ width: '100%', height: '100%', background: '#0f172a' }}>
      <ReactFlow
        nodes={nodes}
        edges={edges}
        onNodesChange={onNodesChange}
        onEdgesChange={onEdgesChange}
        fitView
        attributionPosition="bottom-right"
      >
        <Background color="#334155" gap={16} />
        <Controls style={{ fill: '#fff' }} />
      </ReactFlow>
    </div>
  );
}
