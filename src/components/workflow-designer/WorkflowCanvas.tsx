import { useState, DragEvent } from 'react';
import { NodeLibrary } from './NodeLibrary';
import { ConnectionManager } from './ConnectionManager';
import { ExecutionMonitor } from './ExecutionMonitor';

export default function WorkflowCanvas() {
  const [nodes, setNodes] = useState<
    { id: string; type: string; label: string; x: number; y: number }[]
  >([
    { id: '1', type: 'trigger', label: 'Start (Webhook)', x: 50, y: 50 },
    { id: '2', type: 'action', label: 'Build Docker', x: 300, y: 150 },
  ]);

  const [connections] = useState([{ id: 'c1', from: '1', to: '2' }]);

  const handleDrop = (e: DragEvent) => {
    e.preventDefault();
    const type = e.dataTransfer.getData('workflowNodeType');

    if (type) {
      // Calcul de la position relative au canvas
      const rect = e.currentTarget.getBoundingClientRect();
      const x = e.clientX - rect.left - 75; // Centrer
      const y = e.clientY - rect.top - 30;

      const newNode = {
        id: Date.now().toString(),
        type,
        label: `Nouveau ${type}`,
        x,
        y,
      };
      setNodes([...nodes, newNode]);
    }
  };

  const handleDragOver = (e: DragEvent) => e.preventDefault();

  return (
    <div style={{ display: 'flex', flexDirection: 'column', height: '100%' }}>
      {/* Zone Principale (Bibliothèque + Canvas) */}
      <div style={{ display: 'flex', flex: 1, overflow: 'hidden' }}>
        <NodeLibrary />

        <div
          style={{
            flex: 1,
            position: 'relative',
            backgroundColor: 'var(--bg-app)',
            backgroundImage: 'radial-gradient(var(--border-color) 1px, transparent 1px)',
            backgroundSize: '24px 24px',
            overflow: 'hidden',
          }}
          onDrop={handleDrop}
          onDragOver={handleDragOver}
        >
          {/* Calque des connexions (SVG) */}
          <ConnectionManager nodes={nodes} connections={connections} />

          {/* Calque des nœuds (HTML) */}
          {nodes.map((node) => (
            <div
              key={node.id}
              style={{
                position: 'absolute',
                left: node.x,
                top: node.y,
                width: '150px',
                padding: 'var(--spacing-2)',
                backgroundColor: 'var(--bg-panel)',
                border: '1px solid var(--border-color)',
                borderRadius: 'var(--radius-md)',
                boxShadow: 'var(--shadow-md)',
                zIndex: 5,
                cursor: 'move',
                display: 'flex',
                alignItems: 'center',
                gap: '8px',
              }}
            >
              <div
                style={{
                  width: '10px',
                  height: '10px',
                  borderRadius: '50%',
                  backgroundColor:
                    node.type === 'trigger'
                      ? 'var(--color-warning)'
                      : node.type === 'end'
                      ? 'var(--color-error)'
                      : 'var(--color-primary)',
                }}
              />
              <span
                style={{
                  fontSize: 'var(--font-size-sm)',
                  fontWeight: 'bold',
                  color: 'var(--text-main)',
                }}
              >
                {node.label}
              </span>
            </div>
          ))}
        </div>
      </div>

      {/* Panneau inférieur */}
      <ExecutionMonitor />
    </div>
  );
}
