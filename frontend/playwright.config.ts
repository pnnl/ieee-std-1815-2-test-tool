import { defineConfig, devices } from '@playwright/test'

/**
 * See https://playwright.dev/docs/test-configuration.
 *
 * PW_BASE_URL overrides the default http://localhost:3001 for local
 * environments where another service (e.g. Grafana) occupies port 3001.
 * Example: PW_BASE_URL=http://localhost:3002 npx playwright test ...
 */
/** Local declaration; the test-layer SoT is `tests/shared_config.ts#PW_BASE_URL`. Keep these in sync. */
const BASE_URL = process.env.PW_BASE_URL ?? 'http://localhost:3001'
const parsedBaseUrl = new URL(BASE_URL)

if (!parsedBaseUrl.port) {
  throw new Error('PW_BASE_URL must include an explicit port')
}

export default defineConfig({
  testDir: './tests',
  // Diagnostic specs use playwright.diag.config.ts; excluded here.
  testIgnore: /-diag\.spec\.ts$/,
  /* Run tests in files in parallel */
  fullyParallel: !process.env.CI,
  /* Fail the build on CI if you accidentally left test.only in the source code. */
  forbidOnly: !!process.env.CI,
  /* Retry on CI only */
  retries: process.env.CI ? 2 : 0,
  /* Opt out of parallel tests on CI. */
  workers: process.env.CI ? 1 : undefined,

  reporter:
    'list' /* Reporter to use. See https://playwright.dev/docs/test-reporters */,
  use: {
    /* Base URL to use in actions like `await page.goto('')`. */
    baseURL: BASE_URL,
    trace: 'on-first-retry',
  },

  projects: [
    {
      name: 'setup',
      testMatch: /global\.setup\.ts/,
    },
    {
      name: 'teardown',
      testMatch: /global\.teardown\.ts/,
    },
    {
      name: 'chromium',
      use: {
        ...devices['Desktop Chrome'],
        storageState: 'storage-state.json',
      },
      dependencies: ['setup'],
      teardown: 'teardown',
    },
  ],

  webServer: {
    command: `npm run dev -- --port ${parsedBaseUrl.port}`,
    url: parsedBaseUrl.origin,
    reuseExistingServer: !process.env.CI,
  },
})
