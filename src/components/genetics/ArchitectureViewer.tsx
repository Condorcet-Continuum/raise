import { useEffect, useRef, useState, useMemo, useCallback } from 'react';

interface Props {
  allocation: [string, string][]; // [FuncID, CompID]
  flows: { source: string; target: string; volume: number }[];
  functionLoads?: Record<string, number>;
  componentCapacities?: Record<string, number>;
}

interface Line {
  x1: number;
  y1: number;
  x2: number;
  y2: number;
  color: string;
  width: number;
  opacity: number;
}

export default function ArchitectureViewer({
  allocation,
  flows,
  functionLoads,
  componentCapacities,
}: Props) {
  const hierarchy = useMemo(() => {
    const map = new Map<string, string[]>();
    allocation.forEach(([f, c]) => {
      if (!map.has(c)) map.set(c, []);
      map.get(c)?.push(f);
    });
    return map;
  }, [allocation]);

  const containerRef = useRef<HTMLDivElement>(null);
  const itemRefs = useRef<Record<string, HTMLDivElement | null>>({});
  const [lines, setLines] = useState<Line[]>([]);

  const calculateLines = useCallback(() => {
    if (!containerRef.current) return;
    const containerRect = containerRef.current.getBoundingClientRect();
    const newLines: Line[] = [];
    const funcToComp = new Map(allocation);

    flows.forEach((flow) => {
      const srcEl = itemRefs.current[flow.source];
      const tgtEl = itemRefs.current[flow.target];

      if (srcEl && tgtEl) {
        const srcRect = srcEl.getBoundingClientRect();
        const tgtRect = tgtEl.getBoundingClientRect();
        const x1 = srcRect.left - containerRect.left + srcRect.width / 2;
        const y1 = srcRect.top - containerRect.top + srcRect.height / 2;
        const x2 = tgtRect.left - containerRect.left + tgtRect.width / 2;
        const y2 = tgtRect.top - containerRect.top + tgtRect.height / 2;

        const isExternal = funcToComp.get(flow.source) !== funcToComp.get(flow.target);

        newLines.push({
          x1,
          y1,
          x2,
          y2,
          color: isExternal ? '#ef4444' : '#555',
          width: isExternal ? 2 : 1,
          opacity: isExternal ? 0.8 : 0.2,
        });
      }
    });
    setLines(newLines);
  }, [allocation, flows]);

  useEffect(() => {
    const timer = setTimeout(calculateLines, 100);
    window.addEventListener('resize', calculateLines);
    return () => {
      window.removeEventListener('resize', calculateLines);
      clearTimeout(timer);
    };
  }, [calculateLines, hierarchy]);

  if (allocation.length === 0)
    return <div style={{ padding: '20px', color: '#888' }}>Aucune allocation à afficher</div>;

  return (
    <div
      ref={containerRef}
      style={{
        position: 'relative',
        minHeight: '400px',
        padding: '40px',
        backgroundColor: '#1e1e1e',
        borderRadius: '8px',
        overflow: 'hidden',
        border: '1px solid #333',
      }}
    >
      <div
        style={{
          position: 'absolute',
          top: 0,
          left: 0,
          width: '100%',
          height: '100%',
          backgroundImage: 'radial-gradient(#333 1px, transparent 1px)',
          backgroundSize: '20px 20px',
          opacity: 0.3,
          pointerEvents: 'none',
        }}
      ></div>

      <svg
        style={{
          position: 'absolute',
          top: 0,
          left: 0,
          width: '100%',
          height: '100%',
          pointerEvents: 'none',
          zIndex: 1,
        }}
      >
        <defs>
          <marker id="head-red" markerWidth="10" markerHeight="10" refX="10" refY="3" orient="auto">
            <path d="M0,0 L0,6 L9,3 z" fill="#ef4444" />
          </marker>
          <marker id="head-gray" markerWidth="6" markerHeight="6" refX="6" refY="3" orient="auto">
            <path d="M0,0 L0,6 L9,3 z" fill="#555" />
          </marker>
        </defs>
        {lines.map((l, i) => (
          <path
            key={i}
            d={`M ${l.x1} ${l.y1} C ${l.x1} ${l.y1 + 50}, ${l.x2} ${l.y2 - 50}, ${l.x2} ${l.y2}`}
            stroke={l.color}
            strokeWidth={l.width}
            fill="none"
            opacity={l.opacity}
            markerEnd={l.color === '#ef4444' ? 'url(#head-red)' : 'url(#head-gray)'}
          />
        ))}
      </svg>

      <div
        style={{
          display: 'flex',
          flexWrap: 'wrap',
          gap: '50px',
          position: 'relative',
          zIndex: 2,
          justifyContent: 'center',
        }}
      >
        {Array.from(hierarchy.entries()).map(([compId, funcs]) => {
          const currentLoad = funcs.reduce((acc, f) => acc + (functionLoads?.[f] || 0), 0);
          const capacity = componentCapacities?.[compId] || 100;
          const loadPerc = Math.min(100, (currentLoad / capacity) * 100);
          const isOverloaded = currentLoad > capacity;

          return (
            <div
              key={compId}
              style={{
                border: isOverloaded ? '2px solid #ef4444' : '1px solid #555',
                backgroundColor: '#252525',
                borderRadius: '6px',
                padding: '10px',
                minWidth: '180px',
                boxShadow: '0 10px 15px -3px rgba(0, 0, 0, 0.5)',
              }}
            >
              <div
                style={{
                  borderBottom: '1px solid #444',
                  paddingBottom: '8px',
                  marginBottom: '8px',
                  display: 'flex',
                  justifyContent: 'space-between',
                  alignItems: 'center',
                }}
              >
                <span style={{ fontWeight: 'bold', fontSize: '13px', color: '#fff' }}>
                  🖥️ {compId}
                </span>
                <div style={{ fontSize: '10px', color: isOverloaded ? '#ef4444' : '#aaa' }}>
                  {currentLoad.toFixed(0)}/{capacity} ({loadPerc.toFixed(0)}%)
                </div>
              </div>
              <div
                style={{
                  height: '4px',
                  width: '100%',
                  background: '#333',
                  marginBottom: '10px',
                  borderRadius: '2px',
                  overflow: 'hidden',
                }}
              >
                <div
                  style={{
                    height: '100%',
                    width: `${loadPerc}%`,
                    background: isOverloaded ? '#ef4444' : '#10b981',
                  }}
                ></div>
              </div>
              <div style={{ display: 'flex', flexDirection: 'column', gap: '6px' }}>
                {funcs.map((fId) => (
                  <div
                    key={fId}
                    ref={(el) => {
                      itemRefs.current[fId] = el;
                    }}
                    style={{
                      padding: '6px',
                      backgroundColor: '#333',
                      borderRadius: '4px',
                      fontSize: '11px',
                      color: '#eee',
                      border: '1px solid #444',
                      display: 'flex',
                      justifyContent: 'space-between',
                    }}
                  >
                    <span>⚙️ {fId}</span>
                    <span style={{ opacity: 0.5 }}>{functionLoads?.[fId] || '?'}</span>
                  </div>
                ))}
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
}
