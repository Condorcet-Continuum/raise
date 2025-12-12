import type { ReactNode } from 'react';

interface SplitPaneProps {
  left: ReactNode;
  right: ReactNode;
  ratio?: number; // 0..1
}

export function SplitPane({ left, right, ratio = 0.5 }: SplitPaneProps) {
  const leftWidth = `${ratio * 100}%`;
  const rightWidth = `${(1 - ratio) * 100}%`;

  return (
    <div
      style={{
        display: 'flex',
        width: '100%',
        height: '100%',
        gap: 'var(--spacing-2)', // Était "gap: 8"
      }}
    >
      <div style={{ flexBasis: leftWidth, flexGrow: 0, flexShrink: 0 }}>{left}</div>
      <div
        style={{
          flexBasis: rightWidth,
          flexGrow: 0,
          flexShrink: 0,
        }}
      >
        {right}
      </div>
    </div>
  );
}
