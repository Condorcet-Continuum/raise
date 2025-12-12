import type { ReactNode } from 'react';

interface ModalProps {
  open: boolean;
  title?: string;
  onClose: () => void;
  children: ReactNode;
}

export function Modal({ open, title, onClose, children }: ModalProps) {
  if (!open) return null;

  return (
    <div
      style={{
        position: 'fixed',
        inset: 0,
        backgroundColor: 'rgba(0, 0, 0, 0.5)', // Backdrop semi-transparent
        backdropFilter: 'blur(2px)', // Petit effet moderne
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        zIndex: 'var(--z-index-modal)',
      }}
      onClick={onClose}
    >
      <div
        style={{
          minWidth: 320,
          maxWidth: 640,
          width: '100%',
          backgroundColor: 'var(--color-white)',
          borderRadius: 'var(--radius-lg)',
          border: '1px solid var(--color-gray-200)',
          padding: 'var(--spacing-4)',
          boxShadow: 'var(--shadow-xl)',
          color: 'var(--color-gray-900)',
        }}
        onClick={(e) => e.stopPropagation()}
      >
        {title && (
          <header
            style={{
              display: 'flex',
              justifyContent: 'space-between',
              alignItems: 'center',
              marginBottom: 'var(--spacing-4)',
              borderBottom: '1px solid var(--color-gray-100)',
              paddingBottom: 'var(--spacing-2)',
            }}
          >
            <h3 style={{ margin: 0, fontSize: 'var(--font-size-lg)' }}>{title}</h3>
            <button
              type="button"
              onClick={onClose}
              style={{
                border: 'none',
                background: 'transparent',
                color: 'var(--color-gray-400)',
                cursor: 'pointer',
                fontSize: '1.2rem',
              }}
            >
              ✕
            </button>
          </header>
        )}
        {children}
      </div>
    </div>
  );
}
