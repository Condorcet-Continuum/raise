import type { ButtonHTMLAttributes, ReactNode } from 'react';

type ButtonVariant = 'primary' | 'secondary' | 'ghost';

interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: ButtonVariant;
  children: ReactNode;
}

export function Button({ variant = 'primary', children, style, ...rest }: ButtonProps) {
  const base: React.CSSProperties = {
    borderRadius: 'var(--radius-full)', // Utilisation de la variable radius
    padding: '6px 12px',
    fontSize: 'var(--font-size-sm)',
    border: '1px solid transparent',
    cursor: 'pointer',
    transition: 'var(--transition-base)', // Ajout de transition
    fontFamily: 'var(--font-family)',
  };

  const palette: Record<ButtonVariant, React.CSSProperties> = {
    primary: {
      backgroundColor: 'var(--color-primary)',
      color: '#ffffff', // On garde blanc pur pour le contraste sur le bleu
      borderColor: 'var(--color-primary-dark)',
    },
    secondary: {
      backgroundColor: 'var(--color-white)', // S'inversera en mode sombre
      color: 'var(--color-gray-900)', // S'inversera en mode sombre
      borderColor: 'var(--color-gray-200)',
    },
    ghost: {
      backgroundColor: 'transparent',
      color: 'var(--color-gray-500)',
      borderColor: 'transparent',
    },
  };

  return (
    <button style={{ ...base, ...palette[variant], ...style }} {...rest}>
      {children}
    </button>
  );
}
