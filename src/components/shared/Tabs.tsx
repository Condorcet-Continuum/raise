import { useState } from 'react';
import type { ReactNode } from 'react';

export interface TabItem {
  id: string;
  label: string;
  content: ReactNode;
}

interface TabsProps {
  items: TabItem[];
  initialId?: string;
}

export function Tabs({ items, initialId }: TabsProps) {
  const [activeId, setActiveId] = useState(() => initialId ?? items[0]?.id);

  const active = items.find((t) => t.id === activeId) ?? items[0];

  return (
    <div>
      <div
        style={{
          display: 'flex',
          gap: 'var(--spacing-2)',
          borderBottom: '1px solid var(--color-gray-200)',
          marginBottom: 'var(--spacing-4)',
        }}
      >
        {items.map((tab) => {
          const isActive = tab.id === active?.id;
          return (
            <button
              key={tab.id}
              type="button"
              onClick={() => setActiveId(tab.id)}
              style={{
                border: 'none',
                background: 'transparent',
                padding: 'var(--spacing-2) var(--spacing-4)',
                cursor: 'pointer',
                fontSize: 'var(--font-size-sm)',
                fontFamily: 'var(--font-family)',
                transition: 'all 0.2s',
                // Logique de couleur conditionnelle
                color: isActive ? 'var(--color-primary)' : 'var(--color-gray-500)',
                borderBottom: isActive ? '2px solid var(--color-primary)' : '2px solid transparent',
                fontWeight: isActive ? 'var(--font-weight-semibold)' : 'var(--font-weight-normal)',
              }}
            >
              {tab.label}
            </button>
          );
        })}
      </div>
      <div style={{ color: 'var(--color-gray-900)' }}>{active?.content}</div>
    </div>
  );
}
