import { defineConfig, devices } from '@playwright/test'

// Standalone diagnostic config: targets running dev server on :3000,
// no setup/teardown, no webServer.
export default defineConfig({
  testDir: './tests',
  testMatch: /test-runner-start-diag\.spec\.ts/,
  fullyParallel: false,
  forbidOnly: false,
  retries: 0,
  workers: 1,
  reporter: 'list',
  timeout: 60000,
  use: {
    baseURL: 'http://localhost:3000',
    trace: 'off',
    storageState: undefined,
  },
  projects: [
    {
      name: 'chromium',
      use: { ...devices['Desktop Chrome'] },
    },
  ],
})
