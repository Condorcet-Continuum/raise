import type { ReactNode } from 'react';

interface CardProps {
  title?: string;
  children: ReactNode;
}

export function Card({ title, children }: CardProps) {
  return (
    <section
      style={{
        borderRadius: 'var(--radius-md)',
        border: '1px solid var(--color-gray-200)',
        backgroundColor: 'var(--color-white)',
        padding: 'var(--spacing-4)',
        color: 'var(--color-gray-900)',
        boxShadow: 'var(--shadow-sm)',
      }}
    >
      {title && (
        <h3
          style={{
            fontSize: 'var(--font-size-sm)',
            fontWeight: 'var(--font-weight-semibold)',
            margin: 0,
            marginBottom: 'var(--spacing-2)',
            color: 'var(--color-gray-900)',
          }}
        >
          {title}
        </h3>
      )}
      <div style={{ color: 'var(--color-gray-500)' }}>{children}</div>
    </section>
  );
}
