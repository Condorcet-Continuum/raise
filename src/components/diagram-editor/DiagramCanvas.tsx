import { useState, DragEvent } from 'react';
import { ShapeLibrary } from './ShapeLibrary';
import { ConnectionTool } from './ConnectionTool';
import { LayoutEngine } from './LayoutEngine';

export default function DiagramCanvas() {
  // État simple pour démo : liste des noeuds déposés
  const [nodes, setNodes] = useState<{ id: number; type: string; x: number; y: number }[]>([]);

  const handleDrop = (e: DragEvent) => {
    e.preventDefault();
    const type = e.dataTransfer.getData('shapeType');
    if (type) {
      // Position relative simple (à améliorer avec getBoundingClientRect)
      const x = e.clientX - 300; // Offset approximatif sidebar
      const y = e.clientY - 100; // Offset approximatif header

      setNodes([...nodes, { id: Date.now(), type, x, y }]);
    }
  };

  const handleDragOver = (e: DragEvent) => e.preventDefault();

  return (
    <div
      style={{
        display: 'flex',
        height: '100%',
        width: '100%',
        backgroundColor: 'var(--bg-app)', // Fond global
        overflow: 'hidden',
      }}
    >
      {/* 1. La Bibliothèque à gauche */}
      <ShapeLibrary />

      {/* 2. La Zone de Dessin */}
      <div
        style={{
          position: 'relative',
          flex: 1,
          height: '100%',
          overflow: 'hidden',
        }}
        onDrop={handleDrop}
        onDragOver={handleDragOver}
      >
        {/* Fond quadrillé CSS adaptatif */}
        <div
          style={{
            position: 'absolute',
            inset: 0,
            opacity: 0.1, // Discret
            backgroundImage: `
            linear-gradient(var(--text-main) 1px, transparent 1px),
            linear-gradient(90deg, var(--text-main) 1px, transparent 1px)
          `,
            backgroundSize: '20px 20px',
            pointerEvents: 'none',
          }}
        />

        {/* 3. Les outils flottants */}
        <ConnectionTool />
        <LayoutEngine />

        {/* 4. Les Noeuds (Rendu des formes) */}
        {nodes.map((node) => (
          <div
            key={node.id}
            style={{
              position: 'absolute',
              left: node.x,
              top: node.y,
              width: 100,
              height: 60,
              backgroundColor: 'var(--bg-panel)',
              border: '2px solid var(--color-primary)',
              borderRadius: 'var(--radius-sm)',
              boxShadow: 'var(--shadow-md)',
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'center',
              color: 'var(--text-main)',
              fontWeight: 'var(--font-weight-medium)',
              fontSize: 'var(--font-size-sm)',
              cursor: 'move',
              zIndex: 10,
            }}
          >
            {node.type}
          </div>
        ))}

        {nodes.length === 0 && (
          <div
            style={{
              position: 'absolute',
              top: '50%',
              left: '50%',
              transform: 'translate(-50%, -50%)',
              color: 'var(--text-muted)',
              textAlign: 'center',
              pointerEvents: 'none',
            }}
          >
            <h3>Espace de modélisation vide</h3>
            <p>Glissez des éléments depuis la bibliothèque</p>
          </div>
        )}
      </div>
    </div>
  );
}
