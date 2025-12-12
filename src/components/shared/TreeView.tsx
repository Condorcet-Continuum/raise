import { useState } from 'react';

export interface TreeNode {
  id: string;
  label: React.ReactNode;
  icon?: string;
  children?: TreeNode[];
  isExpanded?: boolean; // État initial optionnel
}

interface TreeViewProps {
  nodes: TreeNode[];
  onSelect?: (nodeId: string) => void;
}

export function TreeView({ nodes, onSelect }: TreeViewProps) {
  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: '2px' }}>
      {nodes.map((node) => (
        <TreeNodeItem key={node.id} node={node} onSelect={onSelect} />
      ))}
    </div>
  );
}

function TreeNodeItem({ node, onSelect }: { node: TreeNode; onSelect?: (id: string) => void }) {
  const [isOpen, setIsOpen] = useState(node.isExpanded ?? false);
  const hasChildren = node.children && node.children.length > 0;

  const handleToggle = (e: React.MouseEvent) => {
    e.stopPropagation();
    setIsOpen(!isOpen);
  };

  const handleClick = () => {
    if (onSelect) onSelect(node.id);
    if (hasChildren && !onSelect) setIsOpen(!isOpen); // Si pas de sélection, le clic toggle
  };

  return (
    <div style={{ paddingLeft: '12px' }}>
      {' '}
      {/* Indentation */}
      <div
        onClick={handleClick}
        style={{
          display: 'flex',
          alignItems: 'center',
          gap: '6px',
          padding: '4px 6px',
          borderRadius: 'var(--radius-sm)',
          cursor: 'pointer',
          color: 'var(--text-main)',
          fontSize: 'var(--font-size-sm)',
          transition: 'background-color 0.1s',
          userSelect: 'none',
        }}
        onMouseEnter={(e) => (e.currentTarget.style.backgroundColor = 'var(--bg-app)')}
        onMouseLeave={(e) => (e.currentTarget.style.backgroundColor = 'transparent')}
      >
        {/* Chevron de pliage */}
        <span
          onClick={hasChildren ? handleToggle : undefined}
          style={{
            width: '16px',
            height: '16px',
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
            fontSize: '0.7rem',
            color: 'var(--text-muted)',
            cursor: hasChildren ? 'pointer' : 'default',
            transform: isOpen ? 'rotate(90deg)' : 'rotate(0deg)',
            transition: 'transform 0.2s',
          }}
        >
          {hasChildren ? '▶' : '•'}
        </span>

        {/* Icône optionnelle */}
        {node.icon && <span>{node.icon}</span>}

        {/* Libellé */}
        <span style={{ whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' }}>
          {node.label}
        </span>
      </div>
      {/* Rendu récursif des enfants */}
      {hasChildren && isOpen && (
        <div style={{ borderLeft: '1px solid var(--border-color)', marginLeft: '7px' }}>
          <TreeView nodes={node.children!} onSelect={onSelect} />
        </div>
      )}
    </div>
  );
}
