interface Node {
  id: string;
  x: number;
  y: number;
  // ... autres props
}

interface Connection {
  id: string;
  from: string;
  to: string;
}

interface ConnectionManagerProps {
  nodes: Node[];
  connections: Connection[];
}

export function ConnectionManager({ nodes, connections }: ConnectionManagerProps) {
  // Fonction pour trouver les coordonnées d'un nœud
  const getNodePos = (id: string) => nodes.find((n) => n.id === id);

  return (
    <svg
      style={{
        position: 'absolute',
        top: 0,
        left: 0,
        width: '100%',
        height: '100%',
        pointerEvents: 'none', // Laisse passer les clics vers le canvas
        zIndex: 0,
      }}
    >
      <defs>
        <marker id="arrowhead" markerWidth="10" markerHeight="7" refX="9" refY="3.5" orient="auto">
          <polygon points="0 0, 10 3.5, 0 7" fill="var(--color-gray-400)" />
        </marker>
      </defs>

      {connections.map((conn) => {
        const start = getNodePos(conn.from);
        const end = getNodePos(conn.to);

        if (!start || !end) return null;

        // On calcule le centre des boîtes (supposons 150x60)
        const x1 = start.x + 150;
        const y1 = start.y + 30;
        const x2 = end.x;
        const y2 = end.y + 30;

        // Courbe de Bézier pour un rendu fluide
        const path = `M ${x1} ${y1} C ${x1 + 50} ${y1}, ${x2 - 50} ${y2}, ${x2} ${y2}`;

        return (
          <g key={conn.id}>
            {/* Ligne principale */}
            <path
              d={path}
              stroke="var(--color-gray-400)"
              strokeWidth="2"
              fill="none"
              markerEnd="url(#arrowhead)"
            />
          </g>
        );
      })}
    </svg>
  );
}
