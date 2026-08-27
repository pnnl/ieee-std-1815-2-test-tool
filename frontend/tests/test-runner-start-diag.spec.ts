import { test, expect, request as pwRequest } from '@playwright/test'

/**
 * One-off diagnostic for issue #222 / Test Runner Start button.
 *
 * Reproduces the click -> SSE -> render flow in a real browser and captures:
 *  - All console output (log/warn/error)
 *  - All page errors (uncaught exceptions)
 *  - All SSE events received via CDP Network.eventSourceMessageReceived
 *  - DOM state of the log panel after 10 seconds of streaming
 *
 * NOTE: live dev stack is on port 3000 (not 3001 from playwright.config).
 * We override baseURL inline and bypass the playwright.config webServer
 * by hitting the running dev server directly.
 */

const FRONTEND_URL = 'http://localhost:3000'
const PROFILE_NAME = 'full' // seed profile, always present

test.describe.configure({ mode: 'serial' })

test('Test Runner Start button SSE diagnostic', async ({ browser }) => {
  // Fresh context that ignores any existing storage state, points at 3000.
  const context = await browser.newContext({
    baseURL: FRONTEND_URL,
    storageState: undefined,
  })
  const page = await context.newPage()

  const consoleMessages: string[] = []
  const pageErrors: string[] = []
  const sseEvents: Array<{ data: string; time: number }> = []
  let sseStreamUrl: string | null = null

  page.on('console', (m) => {
    consoleMessages.push(`${m.type()}: ${m.text()}`)
  })
  page.on('pageerror', (e) => {
    pageErrors.push(`${e.message}\n${e.stack ?? ''}`)
  })

  // CDP for SSE capture
  const cdp = await context.newCDPSession(page)
  await cdp.send('Network.enable')
  cdp.on('Network.eventSourceMessageReceived', (e) => {
    sseEvents.push({ data: e.data, time: e.timestamp })
  })
  cdp.on('Network.responseReceived', (e) => {
    const url: string = e.response?.url ?? ''
    if (url.includes('/api/jobs/') && url.endsWith('/events')) {
      sseStreamUrl = url
    }
  })

  // Pre-seed localStorage so default-profile-load fires for 'full'.
  // Need to navigate to origin first to access localStorage.
  await page.goto('/')
  await page.evaluate((name) => {
    localStorage.setItem('mesa.lastLoadedProfile', name)
  }, PROFILE_NAME)
  await page.reload()

  // Wait for the header to indicate the profile loaded. We look for any
  // element that includes the profile name; if that fails, fall back to
  // waiting for the Test Runner tab to be visible.
  await page.waitForLoadState('networkidle')
  const testRunnerTab = page.getByRole('tab', { name: 'Test Runner' })
  await expect(testRunnerTab).toBeVisible({ timeout: 15000 })

  // Click Test Runner tab.
  await testRunnerTab.click()

  // Find Start button. It's labeled "Start Test Run" with aria-label.
  const startBtn = page.getByRole('button', { name: /Start test run/i })
  await expect(startBtn).toBeVisible({ timeout: 10000 })
  await expect(startBtn).toBeEnabled({ timeout: 10000 })

  console.log('[DIAG] About to click Start')
  await startBtn.click()

  // Stream for 10 seconds.
  await page.waitForTimeout(10000)

  // Capture log panel state
  const logEntryLocator = page.locator('[class*="logEntry"]')
  const logEntryCount = await logEntryLocator.count()
  let allLogTexts: string[] = []
  try {
    allLogTexts = await logEntryLocator.allInnerTexts()
  } catch (e) {
    allLogTexts = [`<failed to read inner texts: ${(e as Error).message}>`]
  }

  // Capture jobId from the URL (best effort) and stop the job.
  let jobId: string | null = null
  if (sseStreamUrl) {
    const m = sseStreamUrl.match(/\/api\/jobs\/([^/]+)\/events/)
    if (m) jobId = m[1]
  }

  // Stop via DELETE so we don't leave a job running.
  if (jobId) {
    try {
      const api = await pwRequest.newContext({ baseURL: FRONTEND_URL })
      const resp = await api.delete(`/api/jobs/${jobId}`)
      console.log(`[DIAG] DELETE /api/jobs/${jobId} -> ${resp.status()}`)
      await api.dispose()
    } catch (e) {
      console.log(`[DIAG] DELETE failed: ${(e as Error).message}`)
    }
  } else {
    console.log('[DIAG] No jobId captured; nothing to DELETE')
  }

  // Final report block
  console.log('\n========== DIAGNOSTIC REPORT ==========')
  console.log(`SSE stream URL:        ${sseStreamUrl}`)
  console.log(`SSE events received:   ${sseEvents.length}`)
  console.log(`Log entries rendered:  ${logEntryCount}`)
  console.log(`Console messages:      ${consoleMessages.length}`)
  console.log(`Page errors:           ${pageErrors.length}`)
  console.log('\n----- SSE EVENTS (first 30) -----')
  sseEvents.slice(0, 30).forEach((e, i) => {
    const truncated =
      e.data.length > 200 ? e.data.slice(0, 200) + '...' : e.data
    console.log(`  [${i}] ${truncated}`)
  })
  console.log('\n----- RENDERED LOG ENTRIES -----')
  allLogTexts.forEach((t, i) => console.log(`  [${i}] ${t}`))
  console.log('\n----- CONSOLE MESSAGES -----')
  consoleMessages.forEach((m, i) => console.log(`  [${i}] ${m}`))
  console.log('\n----- PAGE ERRORS -----')
  pageErrors.forEach((e, i) => console.log(`  [${i}] ${e}`))
  console.log('========== END REPORT ==========\n')

  await page.close()
  await context.close()

  // Always pass - this is a diagnostic, the value is in the report.
  expect(true).toBe(true)
})
