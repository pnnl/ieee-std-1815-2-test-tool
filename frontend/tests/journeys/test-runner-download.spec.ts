/**
 * test-runner-download.spec.ts
 *
 * End-to-end golden-path for the Test Runner download bundle feature.
 *
 * What this spec covers:
 *   1. Navigate to the Test Runner tab using the global fixture profile
 *      (playwright_test_profile, created by global.setup.ts).
 *   2. Start a conformance run and wait for at least one real result to land
 *      in the results tree, polling the DOM rather than sleeping.
 *   3. Click the single Download button, capture the .zip with Playwright's
 *      waitForEvent('download') pattern.
 *   4. Unzip the archive in Node and assert field values on every entry:
 *        results.json   : schema_version, summary counts, scenario list,
 *                         tri-state `passed` preserved, outcome enum valid
 *        profile.json   : parses, is non-empty
 *        report.md      : non-empty, contains the ## Summary heading and the
 *                         ## Console Log section header
 *   5. Confirm the Download button is disabled before any run starts and
 *      enabled once results have arrived.
 *
 * Fixture strategy: this spec relies on the global setup profile
 * (playwright_test_profile / 'full' seed) that global.setup.ts already
 * creates. The global teardown cleans it up. No extra beforeAll/afterAll
 * profile management is needed here because the profile is used read-only.
 *
 * All scenarios are enabled by default in the app's preference state. The run
 * produces results quickly (observed under 5 seconds on the local dev stack)
 * because the reference binaries are pre-built before Playwright starts
 * (in CI: the "Pre-build reference binaries" workflow step; locally: the
 * binaries are already present in the dev compose target volume).
 * The spec asserts structural correctness of the archive and cross-checks
 * summary counts against the per-scenario test totals; it does not assert
 * which specific tests pass or fail, as those outcomes depend on the
 * reference implementation under test.
 *
 * Node-side unzip uses fflate, the same library the app uses. fflate ships
 * both ESM and CJS entries; the top-level ESM import resolves in Playwright's
 * Node process via the 'node' export condition in fflate's package.json.
 */

import { test, expect } from '@playwright/test'
import * as fs from 'fs'
import * as os from 'os'
import * as path from 'path'
import { unzipSync, strFromU8 } from 'fflate'

// All scenarios are enabled by default in the app (the preferences default
// enables every scenario). The spec does not restrict to a subset; it lets the
// app run whatever scenarios are enabled in its default state. Observed: 9
// scenarios, 217 total tests, all completing within about 5 seconds against the
// reference outstation on the local dev stack.

// The maximum time to wait for at least one result indicator to appear in the
// DOM after clicking Start. The backend flushes all results in one burst once
// the scenario completes (observed ~5-10s in practice on the local stack).
// 90s gives ample headroom for the conformance run itself plus process startup
// on a loaded CI runner. The reference binaries are pre-built in the CI
// "Pre-build reference binaries" step before Playwright starts, so this
// timeout no longer needs to cover an on-demand cargo compile.
const RESULT_WAIT_TIMEOUT_MS = 90_000

// Per-test timeout for the full-run test. Must exceed the sum of:
//   RESULT_WAIT_TIMEOUT_MS (90s)  - wait for the Download button to enable
//   DOWNLOAD_TIMEOUT_MS   (120s)  - wait for the download event
//   navigation and overhead       (~30s)
// CI runners are slower than local machines, so we use 5 minutes to give
// process startup, scenario orchestration, and zip download room to complete.
// The reference binary compile is done in the pre-build CI step, not inside
// this window. The Playwright default of 30s is far too tight for a real
// end-to-end conformance run.
const FULL_RUN_TEST_TIMEOUT_MS = 300_000

// Name of the Download button label text as it appears in the DOM.
const DOWNLOAD_BUTTON_LABEL = /Download/i

// Playwright download event collection timeout: longer than the run itself.
const DOWNLOAD_TIMEOUT_MS = 120_000

// The three entries the zip must contain, no more, no less.
const EXPECTED_ZIP_ENTRIES = [
  'results.json',
  'profile.json',
  'report.md',
] as const

// Valid outcome strings per testOutcome.ts.
type TestOutcome = 'passed' | 'failed' | 'pending'
const VALID_OUTCOMES: ReadonlySet<string> = new Set([
  'passed',
  'failed',
  'pending',
])

// Force serial mode: this spec starts a real conformance run and the backend
// serializes jobs, so parallel execution would collide.
test.describe.configure({ mode: 'serial' })

test.describe('Test Runner: download bundle golden path', () => {
  test('Download button is disabled before any run', async ({ page }) => {
    await page.goto('/')
    // Wait for the bootstrap profile to load.
    await expect(
      page.locator('p', { hasText: /^Current Profile:/ }),
    ).toBeVisible({ timeout: 15_000 })

    await page.getByRole('tab', { name: 'Test Runner' }).click()
    const downloadBtn = page.getByRole('button', {
      name: DOWNLOAD_BUTTON_LABEL,
    })
    await expect(downloadBtn).toBeVisible({ timeout: 10_000 })
    await expect(downloadBtn).toBeDisabled()
  })

  test('full run: start, wait for result, download, assert zip contents', async ({
    page,
  }) => {
    // Override the per-test timeout. The Playwright config default (30s) is
    // far shorter than RESULT_WAIT_TIMEOUT_MS (90s). A real conformance run
    // inside CI requires process startup, TCP port readiness, scenario
    // orchestration, and a zip download. Reference binary compilation is done
    // in the CI "Pre-build reference binaries" step, not inside this window.
    // 5 minutes gives comfortable margin on a slow CI runner while still
    // failing fast if something is genuinely broken.
    test.setTimeout(FULL_RUN_TEST_TIMEOUT_MS)

    // Navigate and load the app.
    await page.goto('/')
    await expect(
      page.locator('p', { hasText: /^Current Profile:/ }),
    ).toBeVisible({ timeout: 15_000 })

    // Navigate to the Test Runner tab.
    await page.getByRole('tab', { name: 'Test Runner' }).click()

    // Before the run starts, the Download button must be disabled.
    const downloadBtn = page.getByRole('button', {
      name: DOWNLOAD_BUTTON_LABEL,
    })
    await expect(downloadBtn).toBeVisible({ timeout: 10_000 })
    await expect(downloadBtn).toBeDisabled()

    // Click Start Test Run.
    const startBtn = page.getByRole('button', { name: /Start test run/i })
    await expect(startBtn).toBeVisible({ timeout: 10_000 })
    await expect(startBtn).toBeEnabled({ timeout: 10_000 })
    await startBtn.click()

    // Wait for at least one conformance result to arrive. The definitive DOM
    // signal is the Download button transitioning from disabled to enabled:
    // TestRunnerTab only enables it when Object.keys(testResults).length > 0.
    // The diag spec used a fixed 10s sleep; we poll the DOM instead.
    await expect(downloadBtn).toBeEnabled({ timeout: RESULT_WAIT_TIMEOUT_MS })

    // Collect the download event BEFORE clicking so the promise is established
    // before the click fires the anchor. This is the mandatory Playwright pattern:
    // Promise.all guarantees no race between waitForEvent and the click.
    const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'mesa-e2e-'))
    const zipPath = path.join(tmpDir, 'bundle.zip')

    const [download] = await Promise.all([
      page.waitForEvent('download', { timeout: DOWNLOAD_TIMEOUT_MS }),
      downloadBtn.click(),
    ])

    // Persist the zip to a temp path we control, so we can read and unzip it.
    await download.saveAs(zipPath)
    expect(fs.existsSync(zipPath)).toBe(true)
    const zipBytes = fs.readFileSync(zipPath)
    expect(zipBytes.length).toBeGreaterThan(0)

    // Unzip using fflate (the same library the app uses). The top-level ESM
    // import resolves in Playwright's Node process because fflate ships both
    // ESM and CJS entries; the Node ESM loader picks the 'node' export
    // condition from fflate's package.json exports map.
    const unzipped = unzipSync(new Uint8Array(zipBytes))

    // Confirm the archive contains EXACTLY the three expected entries.
    const actualEntries = Object.keys(unzipped).sort()
    const expectedSorted = [...EXPECTED_ZIP_ENTRIES].sort()
    expect(actualEntries).toEqual(expectedSorted)

    // ---------------------------------------------------------------------------
    // results.json assertions
    // ---------------------------------------------------------------------------
    const resultsRaw = strFromU8(unzipped['results.json'])
    const results = JSON.parse(resultsRaw) as Record<string, unknown>

    // Top-level required fields.
    expect(typeof results['schema_version']).toBe('number')
    expect(results['schema_version']).toBe(1)
    expect(typeof results['generated_at']).toBe('string')
    // generated_at must be a parseable ISO timestamp.
    const generatedAt = new Date(results['generated_at'] as string)
    expect(isNaN(generatedAt.getTime())).toBe(false)

    // Summary must be an object with four numeric fields.
    const summary = results['summary'] as Record<string, unknown>
    expect(typeof summary).toBe('object')
    expect(typeof summary['total']).toBe('number')
    expect(typeof summary['passed']).toBe('number')
    expect(typeof summary['failed']).toBe('number')
    expect(typeof summary['pending']).toBe('number')

    // Counts must be non-negative and consistent: total = passed + failed + pending.
    const {
      total,
      passed: sumPassed,
      failed: sumFailed,
      pending: sumPending,
    } = summary as {
      total: number
      passed: number
      failed: number
      pending: number
    }
    expect(total).toBeGreaterThanOrEqual(0)
    expect(sumPassed).toBeGreaterThanOrEqual(0)
    expect(sumFailed).toBeGreaterThanOrEqual(0)
    expect(sumPending).toBeGreaterThanOrEqual(0)
    expect(total).toBe(sumPassed + sumFailed + sumPending)

    // The run produced at least one result: total must be > 0.
    expect(total).toBeGreaterThan(0)

    // scenarios array must be present and non-empty.
    const scenarios = results['scenarios'] as unknown[]
    expect(Array.isArray(scenarios)).toBe(true)
    expect(scenarios.length).toBeGreaterThan(0)

    // Inspect the tests inside each scenario.
    let totalTestsInScenarios = 0
    let foundConformanceResult = false

    for (const scenario of scenarios) {
      const s = scenario as Record<string, unknown>
      expect(typeof s['scenario_id']).toBe('string')
      expect(typeof s['scenario_name']).toBe('string')
      expect(Array.isArray(s['tests'])).toBe(true)

      const tests = s['tests'] as Array<Record<string, unknown>>
      totalTestsInScenarios += tests.length

      for (const t of tests) {
        expect(typeof t['test']).toBe('string')
        expect(typeof t['expected_to_pass']).toBe('boolean')

        // outcome must be one of the three valid strings.
        const outcome = t['outcome'] as string
        expect(VALID_OUTCOMES.has(outcome)).toBe(true)

        // passed is boolean | null | undefined (tri-state). It must NOT be
        // coerced from null to false. If present, it must be boolean or null.
        if ('passed' in t && t['passed'] !== undefined) {
          const passedVal = t['passed']
          expect(
            passedVal === true || passedVal === false || passedVal === null,
          ).toBe(true)
        }

        // If passed is a boolean and outcome is not pending, verify the
        // outcome derivation is consistent with testOutcome.ts logic:
        //   passed===true  && expected_to_pass===true  => passed
        //   passed===true  && expected_to_pass===false => failed
        //   passed===false && expected_to_pass===true  => failed
        //   passed===false && expected_to_pass===false => passed
        //   passed===null                              => pending
        if (typeof t['passed'] === 'boolean') {
          const boolPassed = t['passed'] as boolean
          const expectedToPass = t['expected_to_pass'] as boolean
          const expectedOutcome: TestOutcome =
            boolPassed === expectedToPass ? 'passed' : 'failed'
          expect(outcome).toBe(expectedOutcome)
          foundConformanceResult = true
        }
      }
    }

    // The sum of tests across all scenario docs must equal the summary total
    // because computeSummary iterates the same scenario list.
    expect(totalTestsInScenarios).toBe(total)

    // We must have found at least one result with a concrete boolean passed value.
    expect(foundConformanceResult).toBe(true)

    // ---------------------------------------------------------------------------
    // profile.json assertions
    // ---------------------------------------------------------------------------
    const profileRaw = strFromU8(unzipped['profile.json'])
    const profile = JSON.parse(profileRaw) as Record<string, unknown>
    // Non-empty object: must have at least one key.
    expect(Object.keys(profile).length).toBeGreaterThan(0)

    // ---------------------------------------------------------------------------
    // report.md assertions
    // ---------------------------------------------------------------------------
    const reportMd = strFromU8(unzipped['report.md'])
    expect(reportMd.length).toBeGreaterThan(0)

    // Must contain the Summary section header.
    expect(reportMd).toContain('## Summary')

    // Must contain the Console Log section header (logs were captured during the run).
    expect(reportMd).toContain('## Console Log')

    // Summary table must contain the word "Total" (from the Markdown table row).
    expect(reportMd).toContain('| Total |')

    // Clean up the temp directory.
    fs.rmSync(tmpDir, { recursive: true, force: true })
  })
})
