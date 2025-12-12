import { create } from 'zustand';

export type AiRole = 'user' | 'assistant' | 'system';

export interface ChatMessage {
  id: string;
  role: AiRole;
  content: string;
  createdAt: string;
  meta?: Record<string, unknown>;
}

export interface AiStoreState {
  messages: ChatMessage[];
  isThinking: boolean;
  error?: string;

  addMessage: (msg: ChatMessage) => void;
  clear: () => void;
  setThinking: (value: boolean) => void;
  setError: (error?: string) => void;
}

export const useAiStore = create<AiStoreState>((set) => ({
  messages: [],
  isThinking: false,
  error: undefined,

  addMessage: (msg) =>
    set((state) => ({
      messages: [...state.messages, msg],
    })),

  clear: () => set({ messages: [], error: undefined }),

  setThinking: (value) => set({ isThinking: value }),

  setError: (error) => set({ error }),
}));
