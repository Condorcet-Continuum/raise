import { useEffect, useState } from 'react';

export function ThemeToggle() {
  const [theme, setTheme] = useState('light');

  useEffect(() => {
    // Au montage, on récupère le thème stocké ou on met 'light' par défaut
    const savedTheme = localStorage.getItem('app-theme') || 'light';
    setTheme(savedTheme);
    document.documentElement.setAttribute('data-theme', savedTheme);
  }, []);

  const toggleTheme = () => {
    const newTheme = theme === 'light' ? 'dark' : 'light';
    setTheme(newTheme);

    // Application sur la racine HTML pour les variables CSS
    document.documentElement.setAttribute('data-theme', newTheme);
    // Sauvegarde pour les futures visites
    localStorage.setItem('app-theme', newTheme);
  };

  return (
    <button
      type="button"
      onClick={toggleTheme}
      className="theme-toggle-btn"
      aria-label={theme === 'light' ? 'Passer en mode sombre' : 'Passer en mode clair'}
    >
      <span aria-hidden="true">{theme === 'light' ? '🌙' : '☀️'}</span>
      <span style={{ marginLeft: 8 }}>{theme === 'light' ? 'Mode Sombre' : 'Mode Clair'}</span>
    </button>
  );
}
