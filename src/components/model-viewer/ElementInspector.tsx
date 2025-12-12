interface ElementInspectorProps {
  element?: { name: string; type: string; description?: string };
}

export function ElementInspector({ element }: ElementInspectorProps) {
  if (!element) {
    return (
      <div
        style={{
          padding: 'var(--spacing-4)',
          color: 'var(--text-muted)',
          textAlign: 'center',
          fontStyle: 'italic',
          fontSize: 'var(--font-size-sm)',
        }}
      >
        Aucune sélection
      </div>
    );
  }

  return (
    <div
      style={{
        padding: 'var(--spacing-4)',
        backgroundColor: 'var(--bg-panel)',
        height: '100%',
        overflowY: 'auto',
      }}
    >
      <h3
        style={{
          margin: '0 0 var(--spacing-4) 0',
          fontSize: 'var(--font-size-md)',
          borderBottom: '1px solid var(--border-color)',
          paddingBottom: 'var(--spacing-2)',
          color: 'var(--text-main)',
        }}
      >
        Propriétés
      </h3>

      <div style={{ display: 'grid', gap: 'var(--spacing-4)' }}>
        <Property label="Nom" value={element.name} />
        <Property label="Type" value={element.type} />
        <Property label="Description" value={element.description || 'Aucune description'} />
      </div>
    </div>
  );
}

function Property({ label, value }: { label: string; value: string }) {
  return (
    <div>
      <div
        style={{ fontSize: 'var(--font-size-xs)', color: 'var(--text-muted)', marginBottom: '2px' }}
      >
        {label}
      </div>
      <div style={{ fontSize: 'var(--font-size-sm)', color: 'var(--text-main)' }}>{value}</div>
    </div>
  );
}
