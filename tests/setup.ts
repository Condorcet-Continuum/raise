// tests/setup.ts
import '@testing-library/jest-dom';
import { vi } from 'vitest';

// Mock global pour Tauri (patrimoine RAISE)
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));
