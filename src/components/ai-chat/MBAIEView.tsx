import { useState } from 'react';
import { ChatInterface } from './ChatInterface';
import { Button } from '@/components/shared/Button';

type Tab = 'llm' | 'agents' | 'context' | 'nlp';

export default function MBAIEView() {
  const [activeTab, setActiveTab] = useState<Tab>('llm');

  return (
    <div style={{ display: 'flex', flexDirection: 'column', height: '100%' }}>
      {/* --- HEADER TABS --- */}
      <header
        style={{
          padding: 'var(--spacing-4)',
          borderBottom: '1px solid var(--border-color)',
          backgroundColor: 'var(--bg-panel)',
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'space-between',
        }}
      >
        <div>
          <h2 style={{ margin: 0, fontSize: '1.2rem', color: 'var(--text-main)' }}>
            MBAIE Console
          </h2>
          <span style={{ fontSize: '0.8rem', color: 'var(--text-muted)' }}>
            Model-Based AI Engineering
          </span>
        </div>

        <div
          style={{
            display: 'flex',
            gap: '8px',
            background: 'var(--bg-app)',
            padding: '4px',
            borderRadius: 'var(--radius-md)',
          }}
        >
          <TabButton
            id="llm"
            label="LLM / Chat"
            icon="💬"
            active={activeTab}
            onClick={setActiveTab}
          />
          <TabButton
            id="agents"
            label="Agents (Brain)"
            icon="🧠"
            active={activeTab}
            onClick={setActiveTab}
          />
          <TabButton
            id="context"
            label="Contexte (RAG)"
            icon="📚"
            active={activeTab}
            onClick={setActiveTab}
          />
          <TabButton
            id="nlp"
            label="NLP Core"
            icon="🔡"
            active={activeTab}
            onClick={setActiveTab}
          />
        </div>
      </header>

      {/* --- CONTENT AREA --- */}
      <div style={{ flex: 1, overflow: 'hidden', position: 'relative' }}>
        {/* VUE 1 : LLM (Chat Existant) */}
        {activeTab === 'llm' && (
          <div
            style={{
              height: '100%',
              maxWidth: '1000px',
              margin: '0 auto',
              padding: 'var(--spacing-4)',
            }}
          >
            <ChatInterface />
          </div>
        )}

        {/* VUE 2 : AGENTS (Dashboard) */}
        {activeTab === 'agents' && <AgentsView />}

        {/* VUE 3 : CONTEXTE (RAG Debug) */}
        {activeTab === 'context' && <ContextView />}

        {/* VUE 4 : NLP (Tools) */}
        {activeTab === 'nlp' && <NlpView />}
      </div>
    </div>
  );
}

// --- SOUS-COMPOSANTS DE VUES (Basés sur le README Backend) ---

function AgentsView() {
  return (
    <div
      style={{
        padding: 'var(--spacing-8)',
        maxWidth: '1200px',
        margin: '0 auto',
        color: 'var(--text-main)',
      }}
    >
      <h3 style={{ borderBottom: '1px solid var(--border-color)', paddingBottom: '10px' }}>
        Système Multi-Agents
      </h3>
      <p style={{ color: 'var(--text-muted)' }}>
        Le "Cerveau Exécutif" responsable de l'action sur le modèle.
      </p>

      <div
        style={{
          display: 'grid',
          gridTemplateColumns: 'repeat(auto-fit, minmax(300px, 1fr))',
          gap: '20px',
          marginTop: '20px',
        }}
      >
        {/* Intent Classifier */}
        <Card title="Intent Classifier" status="active">
          <p>Analyse la demande utilisateur pour router vers le bon agent.</p>
          <div style={{ fontSize: '0.9rem', marginTop: '10px' }}>
            <div>
              Dernière détection :{' '}
              <code style={{ color: 'var(--color-primary)' }}>CREATE_OA_ACTOR</code>
            </div>
            <div>
              Confiance : <strong style={{ color: 'var(--color-success)' }}>98.5%</strong>
            </div>
          </div>
        </Card>

        {/* System Agent */}
        <Card title="System Agent" status="active">
          <p>Spécialiste OA/SA. Capable de créer des Acteurs, Fonctions et Capacités.</p>
          <ul style={{ paddingLeft: '20px', fontSize: '0.9rem', color: 'var(--text-muted)' }}>
            <li>✅ Création d'éléments</li>
            <li>✅ Enrichissement description (GenAI)</li>
            <li>✅ Insertion JSON-DB</li>
          </ul>
        </Card>

        {/* Software Agent */}
        <Card title="Software Agent" status="planned">
          <p>Spécialiste LA/PA. Génération de code et composants logiques.</p>
          <div style={{ marginTop: '10px', fontStyle: 'italic', color: 'var(--text-muted)' }}>
            🚧 En cours de développement
          </div>
        </Card>

        {/* Hardware Agent */}
        <Card title="Hardware Agent" status="planned">
          <p>Spécialiste Matériel. Allocation physique et contraintes.</p>
          <div style={{ marginTop: '10px', fontStyle: 'italic', color: 'var(--text-muted)' }}>
            📅 Roadmap 2024
          </div>
        </Card>
      </div>
    </div>
  );
}

function ContextView() {
  return (
    <div
      style={{
        padding: 'var(--spacing-8)',
        maxWidth: '1200px',
        margin: '0 auto',
        color: 'var(--text-main)',
      }}
    >
      <h3 style={{ borderBottom: '1px solid var(--border-color)', paddingBottom: '10px' }}>
        Mémoire Contextuelle (RAG)
      </h3>
      <p style={{ color: 'var(--text-muted)' }}>
        Responsable de l'ancrage des réponses dans la réalité du projet.
      </p>

      <div style={{ marginTop: '20px', display: 'flex', flexDirection: 'column', gap: '20px' }}>
        <div
          style={{
            padding: '20px',
            backgroundColor: 'var(--bg-panel)',
            border: '1px solid var(--border-color)',
            borderRadius: 'var(--radius-lg)',
          }}
        >
          <h4 style={{ margin: '0 0 10px 0' }}>État du Retriever</h4>
          <div style={{ display: 'flex', gap: '40px' }}>
            <div>
              Type : <strong>Naïf (In-Memory)</strong>
            </div>
            <div>
              Documents indexés : <strong>42</strong>
            </div>
            <div>
              Dernière sync : <strong>Il y a 2 min</strong>
            </div>
          </div>
        </div>

        <div
          style={{
            padding: '20px',
            backgroundColor: 'var(--bg-panel)',
            border: '1px solid var(--border-color)',
            borderRadius: 'var(--radius-lg)',
          }}
        >
          <h4 style={{ margin: '0 0 10px 0' }}>Contexte Injecté (Dernier Prompt)</h4>
          <pre
            style={{
              background: 'var(--bg-app)',
              padding: '15px',
              borderRadius: 'var(--radius-md)',
              fontSize: '0.85rem',
              overflowX: 'auto',
              color: 'var(--text-muted)',
            }}
          >
            {`[SYSTEM]
Tu es un architecte système expert Arcadia.
Voici les éléments pertinents trouvés dans le projet :
- OperationalActor: "Opérateur Maritime" (id: oa-1)
- OperationalActivity: "Surveiller Zone" (id: act-1)

Réponds à la question de l'utilisateur en utilisant ces éléments.`}
          </pre>
        </div>
      </div>
    </div>
  );
}

function NlpView() {
  return (
    <div
      style={{
        padding: 'var(--spacing-8)',
        maxWidth: '800px',
        margin: '0 auto',
        color: 'var(--text-main)',
      }}
    >
      <h3 style={{ borderBottom: '1px solid var(--border-color)', paddingBottom: '10px' }}>
        NLP Debug Tools
      </h3>

      <div style={{ marginTop: '20px' }}>
        <label style={{ display: 'block', marginBottom: '8px' }}>Test Tokenization</label>
        <div style={{ display: 'flex', gap: '10px' }}>
          <input
            type="text"
            placeholder="Entrez une phrase..."
            style={{
              flex: 1,
              padding: '10px',
              borderRadius: 'var(--radius-md)',
              border: '1px solid var(--border-color)',
              background: 'var(--bg-panel)',
              color: 'var(--text-main)',
            }}
          />
          <Button variant="secondary">Analyser</Button>
        </div>

        <div
          style={{
            marginTop: '20px',
            padding: '15px',
            background: 'var(--bg-panel)',
            borderRadius: 'var(--radius-md)',
            border: '1px dashed var(--border-color)',
          }}
        >
          <div style={{ color: 'var(--text-muted)', textAlign: 'center' }}>
            Résultats (Tokens, Embeddings) s'afficheront ici...
          </div>
        </div>
      </div>
    </div>
  );
}

// --- UTILS ---

function TabButton({
  id,
  label,
  icon,
  active,
  onClick,
}: {
  id: Tab;
  label: string;
  icon: string;
  active: Tab;
  onClick: (t: Tab) => void;
}) {
  const isActive = active === id;
  return (
    <button
      onClick={() => onClick(id)}
      style={{
        display: 'flex',
        alignItems: 'center',
        gap: '8px',
        padding: '8px 16px',
        border: 'none',
        borderRadius: 'var(--radius-sm)',
        background: isActive ? 'var(--bg-panel)' : 'transparent',
        color: isActive ? 'var(--color-primary)' : 'var(--text-muted)',
        fontWeight: isActive ? 'bold' : 'normal',
        cursor: 'pointer',
        boxShadow: isActive ? 'var(--shadow-sm)' : 'none',
        transition: 'all 0.2s',
      }}
    >
      <span>{icon}</span>
      {label}
    </button>
  );
}

function Card({
  title,
  status,
  children,
}: {
  title: string;
  status: 'active' | 'planned';
  children: React.ReactNode;
}) {
  const statusColor = status === 'active' ? 'var(--color-success)' : 'var(--color-warning)';
  const statusLabel = status === 'active' ? 'ACTIF' : 'PRÉVU';

  return (
    <div
      style={{
        backgroundColor: 'var(--bg-panel)',
        border: '1px solid var(--border-color)',
        borderRadius: 'var(--radius-lg)',
        padding: '20px',
        position: 'relative',
        overflow: 'hidden',
      }}
    >
      <div
        style={{
          position: 'absolute',
          top: 0,
          right: 0,
          background: statusColor,
          color: '#fff',
          fontSize: '0.65rem',
          fontWeight: 'bold',
          padding: '4px 8px',
          borderBottomLeftRadius: '8px',
        }}
      >
        {statusLabel}
      </div>
      <h4 style={{ marginTop: 0, marginBottom: '10px', color: 'var(--color-primary)' }}>{title}</h4>
      <div style={{ fontSize: '0.9rem', lineHeight: '1.5' }}>{children}</div>
    </div>
  );
}
