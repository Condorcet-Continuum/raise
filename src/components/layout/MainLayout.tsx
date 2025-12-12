import { ReactNode } from 'react';
import { Sidebar } from './Sidebar';
import { Header } from './Header';

interface MainLayoutProps {
  children: ReactNode;
  currentPage: string;
  onNavigate: (page: string) => void;
  pageTitle: string;
}

export function MainLayout({ children, currentPage, onNavigate, pageTitle }: MainLayoutProps) {
  return (
    <div
      style={{
        display: 'flex',
        height: '100vh',
        width: '100vw',
        backgroundColor: 'var(--bg-app)',
        color: 'var(--text-main)',
        overflow: 'hidden',
      }}
    >
      <Sidebar currentPage={currentPage} onNavigate={onNavigate} />

      <div style={{ flex: 1, display: 'flex', flexDirection: 'column', height: '100%' }}>
        <Header title={pageTitle} />

        {/* Zone de contenu variable */}
        <main style={{ flex: 1, overflowY: 'auto', position: 'relative' }}>{children}</main>
      </div>
    </div>
  );
}
