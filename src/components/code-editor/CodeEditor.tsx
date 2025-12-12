import { useState, useRef, ChangeEvent } from 'react';
import { CodeCompletion } from './CodeCompletion';

interface CodeEditorProps {
  value: string;
  onChange: (val: string) => void;
  language?: 'json' | 'javascript';
  placeholder?: string;
}

export function CodeEditor({ value, onChange, language = 'json', placeholder }: CodeEditorProps) {
  const [showSuggestions, setShowSuggestions] = useState(false);
  const [cursorPos, setCursorPos] = useState({ top: 0, left: 0 });
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  // Simulation basique : suggestions si on tape guillemet
  const handleChange = (e: ChangeEvent<HTMLTextAreaElement>) => {
    const val = e.target.value;
    onChange(val);

    // Logique très simple pour démo : si le dernier char est '"', on propose des clés
    if (val.slice(-1) === '"') {
      setShowSuggestions(true);
      // On pourrait calculer la vraie position du curseur ici
      setCursorPos({ top: 50, left: 100 });
    } else {
      setShowSuggestions(false);
    }
  };

  // Insertion de la suggestion
  const handleSuggestion = (text: string) => {
    const newVal = value + text + '"';
    onChange(newVal);
    setShowSuggestions(false);
    textareaRef.current?.focus();
  };

  // Calcul des numéros de ligne
  const lineCount = value.split('\n').length;
  const lines = Array.from({ length: Math.max(lineCount, 1) }, (_, i) => i + 1);

  return (
    <div
      style={{
        position: 'relative',
        display: 'flex',
        height: '100%',
        minHeight: '300px',
        backgroundColor: 'var(--bg-panel)',
        border: '1px solid var(--border-color)',
        borderRadius: 'var(--radius-md)',
        overflow: 'hidden',
        fontFamily: 'var(--font-family-mono)',
      }}
    >
      {/* Colonne des numéros de ligne */}
      <div
        style={{
          width: '3rem',
          backgroundColor: 'var(--bg-app)',
          borderRight: '1px solid var(--border-color)',
          color: 'var(--text-muted)',
          fontSize: 'var(--font-size-sm)',
          lineHeight: '1.5',
          textAlign: 'right',
          padding: 'var(--spacing-4) var(--spacing-2)',
          userSelect: 'none',
        }}
      >
        {lines.map((l) => (
          <div key={l}>{l}</div>
        ))}
      </div>

      {/* Zone d'édition */}
      <div style={{ position: 'relative', flex: 1, display: 'flex' }}>
        <textarea
          ref={textareaRef}
          value={value}
          onChange={handleChange}
          placeholder={placeholder}
          spellCheck={false}
          style={{
            width: '100%',
            height: '100%',
            border: 'none',
            resize: 'none',
            outline: 'none',
            padding: 'var(--spacing-4)',
            backgroundColor: 'transparent',
            color: 'var(--text-main)',
            fontSize: 'var(--font-size-sm)',
            lineHeight: '1.5',
            fontFamily: 'inherit',
          }}
        />

        {/* Popup d'autocomplétion */}
        <CodeCompletion
          visible={showSuggestions}
          position={cursorPos}
          suggestions={['nom', 'description', 'version', 'auteur']}
          onSelect={handleSuggestion}
          onClose={() => setShowSuggestions(false)}
        />
      </div>

      {/* Badge de langage en bas à droite */}
      <div
        style={{
          position: 'absolute',
          bottom: 'var(--spacing-2)',
          right: 'var(--spacing-4)',
          fontSize: 'var(--font-size-xs)',
          color: 'var(--text-muted)',
          opacity: 0.7,
          pointerEvents: 'none',
        }}
      >
        {language.toUpperCase()}
      </div>
    </div>
  );
}
