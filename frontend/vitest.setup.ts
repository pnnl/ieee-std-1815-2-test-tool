import '@testing-library/jest-dom/vitest'
import { beforeEach, vi } from 'vitest'

// jsdom has no layout engine; verify panel sizing in a real browser.
beforeEach(() => {
  vi.stubGlobal(
    'ResizeObserver',
    class {
      observe() {}
      unobserve() {}
      disconnect() {}
    },
  )
})
