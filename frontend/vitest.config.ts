import { defineConfig } from 'vitest/config'
import react from '@vitejs/plugin-react'
import path from 'path'
import { fileURLToPath } from 'url'

const __dirname = path.dirname(fileURLToPath(import.meta.url))

export default defineConfig({
  plugins: [react()],
  test: {
    globals: true,
    environment: 'jsdom',
    include: ['src/**/*.{test,spec}.{ts,tsx,js,jsx}'],
    exclude: [
      '**/node_modules/**',
      '**/dist/**',
      'tests/**', // Playwright lives here, not vitest's job
      'src/api/generated/**', // generated SDK
    ],
    setupFiles: ['./vitest.setup.ts'],
    css: true,
    passWithNoTests: true,
  },
  resolve: {
    alias: {
      '@': path.resolve(__dirname, './src'),
    },
  },
})
