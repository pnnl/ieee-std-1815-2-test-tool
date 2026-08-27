/**
 * exportBundle.ts: pure builders and the browser-coupled download trigger
 * for the zip bundle export feature.
 *
 * Architecture:
 *   buildResultsJson      - pure, no browser APIs, unit-testable
 *   buildReportMarkdown   - pure, no browser APIs, unit-testable
 *   triggerBundleDownload - browser-coupled (Blob, URL, anchor, zip write)
 *
 * Zip format (fflate synchronous API):
 *   mesa-test-run-<profile-name>-<ISO-timestamp>.zip
 *     results.json
 *     profile.json
 *     report.md
 */

import { zipSync, strToU8 } from 'fflate'
import type { ConformanceTestResult, LogEntry, SunSpecComment } from './types'
import type { Scenario } from './scenarios'
import type { PicsProfile } from '@/api/generated'
import { deriveOutcome, type TestOutcome } from './testOutcome'
import { formatLogsForDownload, triggerDownloadWithBlob } from './downloadLogs'
import { testNameToControlModeTest } from './expectedTests'

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

export interface BundleInput {
  scenarios: Scenario[]
  results: Record<string, ConformanceTestResult>
  logs: LogEntry[]
  profileData: PicsProfile
  profileName: string | null
  jobId: string | null
  deviceUnderTest: string
  hasStarted: boolean
}

/** Shape of a single test entry in results.json. */
export interface ResultsJsonTest {
  test: string
  display_name: string
  passed: boolean | null | undefined
  expected_to_pass: boolean
  outcome: TestOutcome
  timestamp: number | undefined
  comments: SunSpecComment[]
}

/** Shape of a single scenario entry in results.json. */
export interface ResultsJsonScenario {
  scenario_id: string
  scenario_name: string
  tests: ResultsJsonTest[]
}

/** Shape of the top-level results.json document. */
export interface ResultsJsonDoc {
  schema_version: 1
  generated_at: string
  job_id: string | null
  device_under_test: string
  profile_name: string | null
  summary: {
    total: number
    passed: number
    failed: number
    pending: number
  }
  scenarios: ResultsJsonScenario[]
}

// ---------------------------------------------------------------------------
// computeSummary: shared outcome aggregation used by both builders
// ---------------------------------------------------------------------------

/** Aggregated outcome counts over all tests in a bundle. */
export interface BundleSummary {
  total: number
  passed: number
  failed: number
  pending: number
}

/**
 * Derive the outcome for a single test, given an optional result record and
 * the expected-pass flag. Returns 'pending' when no result has arrived yet.
 * This is the shared one-liner used by computeSummary, buildResultsJson, and
 * buildReportMarkdown so all three cannot diverge.
 */
export function deriveTestOutcome(
  result: ConformanceTestResult | undefined,
  expectedShouldPass: boolean,
): TestOutcome {
  return result ? deriveOutcome(result.passed, expectedShouldPass) : 'pending'
}

/**
 * Iterate every expected test across all scenarios, call `deriveTestOutcome` for
 * each, and return the {total, passed, failed, pending} counts. Both
 * `buildResultsJson` and `buildReportMarkdown` call this so the two output
 * paths cannot diverge from the on-screen tree.
 */
export function computeSummary(
  scenarios: Scenario[],
  results: Record<string, ConformanceTestResult>,
): BundleSummary {
  let passed = 0
  let failed = 0
  let pending = 0

  for (const scenario of scenarios) {
    for (const expected of scenario.expected_tests) {
      const key = `${scenario.id}:${expected.test_id}`
      const result = results[key] as ConformanceTestResult | undefined
      const outcome = deriveTestOutcome(result, expected.should_pass)
      if (outcome === 'passed') passed++
      else if (outcome === 'failed') failed++
      else pending++
    }
  }

  return { total: passed + failed + pending, passed, failed, pending }
}

// ---------------------------------------------------------------------------
// buildResultsJson: pure
// ---------------------------------------------------------------------------

export function buildResultsJson(input: BundleInput): ResultsJsonDoc {
  const { scenarios, results, profileName, jobId, deviceUnderTest } = input

  const summary = computeSummary(scenarios, results)

  const scenarioDocs: ResultsJsonScenario[] = scenarios.map((scenario) => {
    const tests: ResultsJsonTest[] = scenario.expected_tests.map((expected) => {
      const key = `${scenario.id}:${expected.test_id}`
      const result = results[key] as ConformanceTestResult | undefined

      // Preserve the tri-state verbatim: true, false, or null. Undefined
      // means no result has arrived yet (field omitted from the JSON output
      // via JSON.stringify's undefined-omits-the-key behavior).
      const passedValue: boolean | null | undefined = result
        ? result.passed
        : undefined

      const outcome = deriveTestOutcome(result, expected.should_pass)

      return {
        test: expected.test_id,
        display_name: testNameToControlModeTest[expected.test_id].displayName,
        passed: passedValue,
        expected_to_pass: expected.should_pass,
        outcome,
        timestamp: result?.timestamp,
        comments: result?.comments ?? [],
      }
    })

    return {
      scenario_id: scenario.id,
      scenario_name: scenario.name,
      tests,
    }
  })

  return {
    schema_version: 1,
    generated_at: new Date().toISOString(),
    job_id: jobId,
    device_under_test: deviceUnderTest,
    profile_name: profileName,
    summary,
    scenarios: scenarioDocs,
  }
}

// ---------------------------------------------------------------------------
// buildReportMarkdown: pure
// ---------------------------------------------------------------------------

export function buildReportMarkdown(input: BundleInput): string {
  const { profileName, jobId, deviceUnderTest, scenarios, results, logs } =
    input

  const now = new Date().toISOString()
  const title = profileName
    ? `MESA Test Run Report: ${profileName}`
    : 'MESA Test Run Report'

  const lines: string[] = []
  lines.push(`# ${title}`)
  lines.push('')
  lines.push('## Run Details')
  lines.push('')
  lines.push(`- Generated: ${now}`)
  if (jobId) lines.push(`- Job ID: ${jobId}`)
  lines.push(`- Device under test: ${deviceUnderTest}`)
  if (profileName) lines.push(`- Profile: ${profileName}`)
  lines.push('')

  // Summary section.
  const {
    total,
    passed: totalPassed,
    failed: totalFailed,
    pending: totalPending,
  } = computeSummary(scenarios, results)

  lines.push('## Summary')
  lines.push('')
  lines.push(`| Status | Count |`)
  lines.push(`|--------|-------|`)
  lines.push(`| Total | ${total} |`)
  lines.push(`| Passed | ${totalPassed} |`)
  lines.push(`| Failed | ${totalFailed} |`)
  lines.push(`| Pending | ${totalPending} |`)
  lines.push('')

  // Per-scenario results.
  lines.push('## Scenario Results')
  lines.push('')
  for (const scenario of scenarios) {
    lines.push(`### ${scenario.name}`)
    lines.push('')
    for (const expected of scenario.expected_tests) {
      const key = `${scenario.id}:${expected.test_id}`
      const result = results[key] as ConformanceTestResult | undefined
      const outcome = deriveTestOutcome(result, expected.should_pass)
      const icon =
        outcome === 'passed'
          ? 'PASS'
          : outcome === 'failed'
            ? 'FAIL'
            : 'PENDING'
      const expectedLabel = expected.should_pass
        ? 'expected pass'
        : 'expected fail'
      lines.push(`- [${icon}] ${expected.test_id} (${expectedLabel})`)
    }
    lines.push('')
  }

  // Console log section: reuse the existing pure formatter.
  if (logs.length > 0) {
    lines.push('## Console Log')
    lines.push('')
    lines.push('```')
    lines.push(formatLogsForDownload(logs))
    lines.push('```')
    lines.push('')
  }

  return lines.join('\n')
}

// ---------------------------------------------------------------------------
// triggerBundleDownload: browser-coupled
// ---------------------------------------------------------------------------

/**
 * Assemble the zip bytes for the bundle. Exported so tests can round-trip
 * the zip contents without touching browser APIs.
 */
export function buildZipBytes(input: BundleInput): Uint8Array {
  const resultsDoc = buildResultsJson(input)
  const reportMd = buildReportMarkdown(input)

  const resultsJson = JSON.stringify(resultsDoc, null, 2)
  const profileJson = JSON.stringify(input.profileData, null, 2)

  return zipSync({
    'results.json': strToU8(resultsJson),
    'profile.json': strToU8(profileJson),
    'report.md': strToU8(reportMd),
  })
}

/**
 * Build the zip bundle in memory and trigger a browser file download.
 * This is the only function in this module that touches browser APIs.
 *
 * fflate's browser entry returns Uint8Array<ArrayBuffer>, so .buffer is
 * already ArrayBuffer: no cast needed. The comment is kept as a
 * version-awareness note in case a future fflate version changes this.
 */
export function triggerBundleDownload(input: BundleInput): void {
  const zipped = buildZipBytes(input)

  // fflate's browser entry returns Uint8Array whose .buffer is ArrayBuffer at
  // runtime. The TypeScript declaration types it as ArrayBufferLike (which
  // includes SharedArrayBuffer), so a cast is still required to satisfy the
  // strict Blob constructor signature. The cast is safe: fflate never returns
  // a SharedArrayBuffer here.
  const blob = new Blob([zipped.buffer as ArrayBuffer], {
    type: 'application/zip',
  })
  triggerDownloadWithBlob(blob, buildZipFilename(input.profileName))
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function buildZipFilename(profileName: string | null): string {
  // Replace colons and other characters that are invalid in filenames with dashes.
  const stamp = new Date().toISOString().replace(/[:.]/g, '-')
  if (profileName) {
    // Sanitize the profile name to be safe in a filename.
    const safe = profileName.replace(/[^a-zA-Z0-9_-]/g, '_')
    return `mesa-test-run-${safe}-${stamp}.zip`
  }
  return `mesa-test-run-${stamp}.zip`
}
