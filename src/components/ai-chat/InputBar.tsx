import { FormEvent } from 'react';

interface InputBarProps {
  value: string;
  onChange: (value: string) => void;
  onSend: (value: string) => void;
  disabled?: boolean;
  placeholder?: string;
}

export function InputBar({ value, onChange, onSend, disabled, placeholder }: InputBarProps) {
  function handleSubmit(e: FormEvent) {
    e.preventDefault();
    const trimmed = value.trim();
    if (!trimmed) return;
    onSend(trimmed);
  }

  return (
    <form
      onSubmit={handleSubmit}
      style={{
        display: 'flex',
        gap: 'var(--spacing-2)',
        paddingTop: 'var(--spacing-4)',
        borderTop: '1px solid var(--border-color)',
      }}
    >
      <textarea
        value={value}
        placeholder={placeholder ?? 'Posez une question à GenAptitude…'}
        onChange={(e) => onChange(e.target.value)}
        disabled={disabled}
        rows={1}
        style={{
          flex: 1,
          resize: 'none',
          borderRadius: 'var(--radius-md)',
          border: '1px solid var(--border-color)',
          padding: 'var(--spacing-2)',
          fontSize: 'var(--font-size-sm)',
          fontFamily: 'var(--font-family)',
          // Fond légèrement différent du panel pour le contraste
          backgroundColor: 'var(--color-gray-50)',
          color: 'var(--text-main)',
          outline: 'none',
        }}
        // Petit trick pour le focus visible via CSS global ou style inline
        onFocus={(e) => (e.target.style.borderColor = 'var(--color-primary)')}
        onBlur={(e) => (e.target.style.borderColor = 'var(--border-color)')}
      />
      <button
        type="submit"
        disabled={disabled || !value.trim()}
        style={{
          borderRadius: 'var(--radius-full)',
          padding: '8px 20px',
          border: 'none',
          backgroundColor: disabled ? 'var(--color-gray-400)' : 'var(--color-primary)',
          color: '#ffffff',
          fontWeight: 'var(--font-weight-medium)',
          cursor: disabled ? 'not-allowed' : 'pointer',
          transition: 'var(--transition-fast)',
        }}
      >
        Envoyer
      </button>
    </form>
  );
}
