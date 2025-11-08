import { useEffect } from 'react'
import { SchemaService } from '@/services/json-db/schema-service'

export default function App() {
  useEffect(() => {
    const svc = new SchemaService()
    ;(async () => {
      try {
        await svc.getSchema('demo')                  
      } catch {
        await svc.registerSchema('demo', { $id: 'demo', type: 'object' })
        console.log('schema demo enregistré')
      }
      const s = await svc.getSchema('demo')
      console.log('schema demo =', s)
    })().catch(console.error)
  }, [])

  return (
    <main style={{ padding: 16 }}>
      <h1>GenAptitude</h1>
      <p>
        Dark mode :{' '}
        <a href="/pages/dark-mode-demo.html">Dark Mode Demo</a>
      </p>
      <p>
        Charte graphique  :{' '}
        <a href="/pages/charte-graphique.html">Charte graphique Demo</a>
      </p>      
    </main>
  )
}
