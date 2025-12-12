interface ContextDisplayProps {
  messagesCount: number;
}

export function ContextDisplay({ messagesCount }: ContextDisplayProps) {
  return (
    <div
      style={{
        fontSize: 'var(--font-size-xs)',
        color: 'var(--text-muted)',
        marginTop: 4,
      }}
    >
      Session de chat · {messagesCount} message(s)
    </div>
  );
}
