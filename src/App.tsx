// On retire { useEffect } des imports
import './styles/variables.css'
import './styles/globals.css'

import { JsonDbTester } from '@/components/JsonDbTester'

export default function App() {
  return (
    <main className="container">
      <header style={{ marginBottom: 32, textAlign: 'center' }}>
        <h1 className="text-primary">GenAptitude</h1>
        <p className="text-gray">Plateforme d'Ingénierie Multi-Domaines</p>
      </header>

      <JsonDbTester />
      
      <div style={{ marginTop: 32 }}>
        <p>Liens utiles :</p>
        <ul>
            <li><a href="/pages/dark-mode-demo.html">Demo Mode Dark</a></li>
            <li><a href="/pages/charte-graphique.html">Charte Graphique</a></li>
        </ul>
      </div>
    </main>
  )
}