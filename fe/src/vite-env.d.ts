/// <reference types="vite/client" />

declare module '*.js';

declare global {
  interface Window {
    flash: (message: string, type: string, options?: { timeout?: number }) => void;
    loading: {
      count: number;
      start: () => void;
      stop: () => void;
    };
    Alpine?: unknown;
  }
}

export {};
