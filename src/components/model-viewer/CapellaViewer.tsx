import { useState } from 'react';
import { SplitPane } from '@/components/shared/SplitPane';
import { ArcadiaLayerView } from './ArcadiaLayerView';
import { ModelNavigator } from './ModelNavigator';
import { DiagramRenderer } from './DiagramRenderer';
import { ElementInspector } from './ElementInspector';

export default function CapellaViewer() {
  const [activeLayer, setActiveLayer] = useState('la');
  const [selectedElement, setSelectedElement] = useState<any>(null);

  // Layout :
  // [Barre Layer] [Navigateur (20%)] [Diagramme (60%)] [Inspecteur (20%)]

  return (
    <div
      style={{
        display: 'flex',
        height: '100%',
        width: '100%',
        backgroundColor: 'var(--bg-app)',
        overflow: 'hidden',
      }}
    >
      {/* 1. Barre de couches (Fixe à gauche) */}
      <ArcadiaLayerView activeLayer={activeLayer} onLayerSelect={setActiveLayer} />

      {/* 2. Contenu principal avec SplitPane imbriqués */}
      <div style={{ flex: 1, height: '100%', position: 'relative' }}>
        <SplitPane
          ratio={0.25} // Navigateur prend 25%
          left={<ModelNavigator />}
          right={
            <SplitPane
              ratio={0.7} // Diagramme prend 70% du reste
              left={<DiagramRenderer diagramId={`Diagramme ${activeLayer.toUpperCase()}`} />}
              right={
                <ElementInspector
                  element={selectedElement || { name: 'Exemple', type: 'LogicalComponent' }}
                />
              }
            />
          }
        />
      </div>
    </div>
  );
}
