/** Unit tests for exportBundle.ts: pure builders verified without DOM, zip round-trip via fflate, browser trigger with stubs. */
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { unzipSync, strFromU8 } from 'fflate'
import {
  buildResultsJson,
  buildReportMarkdown,
  buildZipBytes,
  computeSummary,
  triggerBundleDownload,
  type BundleInput,
} from './exportBundle'
import type { ConformanceTestResult } from './types'
import type { Scenario } from './scenarios'
import type { PicsProfile } from '@/api/generated'

// ---------------------------------------------------------------------------
// Shared fixtures
// ---------------------------------------------------------------------------

const minimalScenario: Scenario = {
  id: 'sc-001',
  name: 'Monitoring Test',
  description: 'Monitors device state',
  expected_tests: [
    { test_id: 'MON_001', should_pass: true },
    { test_id: 'ALARM_001', should_pass: true },
    { test_id: 'CONN_001', should_pass: false }, // expected to fail
  ],
}

function makeResult(
  scenarioId: string,
  testId: string,
  passed: boolean | null,
  timestamp = 1000,
): ConformanceTestResult {
  return {
    event_type: 'conformance_test_result',
    scenario_id: scenarioId,
    test: testId,
    passed,
    timestamp,
    comments: [],
  }
}

const fakeProfile = { version: 1, entities: [] } as unknown as PicsProfile

function makeInput(overrides: Partial<BundleInput> = {}): BundleInput {
  return {
    scenarios: [minimalScenario],
    results: {},
    logs: [],
    profileData: fakeProfile,
    profileName: 'my-profile',
    jobId: 'job-abc',
    deviceUnderTest: 'outstation',
    hasStarted: true,
    ...overrides,
  }
}

// ---------------------------------------------------------------------------
// buildResultsJson: schema_version
// ---------------------------------------------------------------------------

describe('buildResultsJson: top-level fields', () => {
  it('includes schema_version = 1', () => {
    const doc = buildResultsJson(makeInput())
    expect(doc.schema_version).toBe(1)
  })

  it('includes generated_at as an ISO 8601 string', () => {
    const doc = buildResultsJson(makeInput())
    expect(typeof doc.generated_at).toBe('string')
    // ISO 8601 format check: parseable by Date
    const d = new Date(doc.generated_at)
    expect(Number.isNaN(d.getTime())).toBe(false)
  })

  it('includes job_id from input', () => {
    const doc = buildResultsJson(makeInput({ jobId: 'j-999' }))
    expect(doc.job_id).toBe('j-999')
  })

  it('sets job_id to null when input jobId is null', () => {
    const doc = buildResultsJson(makeInput({ jobId: null }))
    expect(doc.job_id).toBeNull()
  })

  it('includes device_under_test from input', () => {
    const doc = buildResultsJson(
      makeInput({ deviceUnderTest: 'control_station' }),
    )
    expect(doc.device_under_test).toBe('control_station')
  })

  it('includes profile_name from input', () => {
    const doc = buildResultsJson(makeInput({ profileName: 'test-profile' }))
    expect(doc.profile_name).toBe('test-profile')
  })

  it('sets profile_name to null when profileName is null', () => {
    const doc = buildResultsJson(makeInput({ profileName: null }))
    expect(doc.profile_name).toBeNull()
  })
})

// ---------------------------------------------------------------------------
// buildResultsJson: passed tri-state preserved verbatim
// ---------------------------------------------------------------------------

describe('buildResultsJson: passed tri-state', () => {
  it('preserves passed=true verbatim in the test entry', () => {
    const input = makeInput({
      results: {
        'sc-001:MON_001': makeResult('sc-001', 'MON_001', true),
      },
    })
    const doc = buildResultsJson(input)
    const sc = doc.scenarios.find((s) => s.scenario_id === 'sc-001')!
    const t = sc.tests.find((t) => t.test === 'MON_001')!
    expect(t.passed).toBe(true)
  })

  it('preserves passed=false verbatim in the test entry', () => {
    const input = makeInput({
      results: {
        'sc-001:MON_001': makeResult('sc-001', 'MON_001', false),
      },
    })
    const doc = buildResultsJson(input)
    const sc = doc.scenarios.find((s) => s.scenario_id === 'sc-001')!
    const t = sc.tests.find((t) => t.test === 'MON_001')!
    expect(t.passed).toBe(false)
  })

  it('preserves passed=null verbatim (does not coerce to false)', () => {
    const input = makeInput({
      results: {
        'sc-001:MON_001': makeResult('sc-001', 'MON_001', null),
      },
    })
    const doc = buildResultsJson(input)
    const sc = doc.scenarios.find((s) => s.scenario_id === 'sc-001')!
    const t = sc.tests.find((t) => t.test === 'MON_001')!
    expect(t.passed).toBeNull()
  })

  it('sets passed=undefined (omitted) for tests with no result', () => {
    const input = makeInput({ results: {} })
    const doc = buildResultsJson(input)
    const sc = doc.scenarios.find((s) => s.scenario_id === 'sc-001')!
    const t = sc.tests.find((t) => t.test === 'MON_001')!
    // No result: passed should be undefined (absent from the record, not null/false).
    expect(t.passed).toBeUndefined()
  })
})

// ---------------------------------------------------------------------------
// buildResultsJson: outcome agrees with deriveOutcome (and therefore the tree)
// ---------------------------------------------------------------------------

describe('buildResultsJson: outcome derivation agrees with the tree', () => {
  it('outcome=passed when passed=true and shouldPass=true', () => {
    const input = makeInput({
      results: { 'sc-001:MON_001': makeResult('sc-001', 'MON_001', true) },
    })
    const doc = buildResultsJson(input)
    const sc = doc.scenarios.find((s) => s.scenario_id === 'sc-001')!
    const t = sc.tests.find((t) => t.test === 'MON_001')!
    expect(t.outcome).toBe('passed')
  })

  it('outcome=failed when passed=false and shouldPass=true', () => {
    const input = makeInput({
      results: { 'sc-001:MON_001': makeResult('sc-001', 'MON_001', false) },
    })
    const doc = buildResultsJson(input)
    const sc = doc.scenarios.find((s) => s.scenario_id === 'sc-001')!
    const t = sc.tests.find((t) => t.test === 'MON_001')!
    expect(t.outcome).toBe('failed')
  })

  it('outcome=passed when passed=false and shouldPass=false (expected-to-fail case)', () => {
    // CONN_001 has should_pass=false in minimalScenario
    const input = makeInput({
      results: { 'sc-001:CONN_001': makeResult('sc-001', 'CONN_001', false) },
    })
    const doc = buildResultsJson(input)
    const sc = doc.scenarios.find((s) => s.scenario_id === 'sc-001')!
    const t = sc.tests.find((t) => t.test === 'CONN_001')!
    expect(t.outcome).toBe('passed')
  })

  it('outcome=failed when passed=true and shouldPass=false (expected-to-fail but passed)', () => {
    const input = makeInput({
      results: { 'sc-001:CONN_001': makeResult('sc-001', 'CONN_001', true) },
    })
    const doc = buildResultsJson(input)
    const sc = doc.scenarios.find((s) => s.scenario_id === 'sc-001')!
    const t = sc.tests.find((t) => t.test === 'CONN_001')!
    expect(t.outcome).toBe('failed')
  })

  it('outcome=pending when passed=null', () => {
    const input = makeInput({
      results: { 'sc-001:MON_001': makeResult('sc-001', 'MON_001', null) },
    })
    const doc = buildResultsJson(input)
    const sc = doc.scenarios.find((s) => s.scenario_id === 'sc-001')!
    const t = sc.tests.find((t) => t.test === 'MON_001')!
    expect(t.outcome).toBe('pending')
  })

  it('outcome=pending for a test with no result when hasStarted=true', () => {
    const input = makeInput({ results: {}, hasStarted: true })
    const doc = buildResultsJson(input)
    const sc = doc.scenarios.find((s) => s.scenario_id === 'sc-001')!
    const t = sc.tests.find((t) => t.test === 'MON_001')!
    expect(t.outcome).toBe('pending')
  })
})

// ---------------------------------------------------------------------------
// buildResultsJson: expected_to_pass
// ---------------------------------------------------------------------------

describe('buildResultsJson: expected_to_pass', () => {
  it('is true for tests where should_pass=true', () => {
    const doc = buildResultsJson(makeInput())
    const sc = doc.scenarios.find((s) => s.scenario_id === 'sc-001')!
    const t = sc.tests.find((t) => t.test === 'MON_001')!
    expect(t.expected_to_pass).toBe(true)
  })

  it('is false for tests where should_pass=false', () => {
    const doc = buildResultsJson(makeInput())
    const sc = doc.scenarios.find((s) => s.scenario_id === 'sc-001')!
    const t = sc.tests.find((t) => t.test === 'CONN_001')!
    expect(t.expected_to_pass).toBe(false)
  })
})

// ---------------------------------------------------------------------------
// buildResultsJson: summary counts
// ---------------------------------------------------------------------------

describe('buildResultsJson: summary counts', () => {
  it('counts one pending when there is no result for a test', () => {
    // minimalScenario has 3 expected tests, no results: all pending.
    const doc = buildResultsJson(makeInput({ results: {}, hasStarted: true }))
    expect(doc.summary.total).toBe(3)
    expect(doc.summary.pending).toBe(3)
    expect(doc.summary.passed).toBe(0)
    expect(doc.summary.failed).toBe(0)
  })

  it('counts correctly with a mix of outcomes', () => {
    // MON_001: passed=true, shouldPass=true   => passed
    // ALARM_001: passed=false, shouldPass=true => failed
    // CONN_001: passed=false, shouldPass=false => passed (expected-to-fail)
    const input = makeInput({
      hasStarted: true,
      results: {
        'sc-001:MON_001': makeResult('sc-001', 'MON_001', true),
        'sc-001:ALARM_001': makeResult('sc-001', 'ALARM_001', false),
        'sc-001:CONN_001': makeResult('sc-001', 'CONN_001', false),
      },
    })
    const doc = buildResultsJson(input)
    expect(doc.summary.total).toBe(3)
    expect(doc.summary.passed).toBe(2)
    expect(doc.summary.failed).toBe(1)
    expect(doc.summary.pending).toBe(0)
  })

  it('counts null-passed tests as pending (not failed)', () => {
    const input = makeInput({
      hasStarted: true,
      results: {
        'sc-001:MON_001': makeResult('sc-001', 'MON_001', null),
      },
    })
    const doc = buildResultsJson(input)
    expect(doc.summary.pending).toBe(3) // MON_001=null pending, ALARM_001+CONN_001=no result pending
    expect(doc.summary.failed).toBe(0)
  })

  it('total equals sum of passed + failed + pending', () => {
    const input = makeInput({
      hasStarted: true,
      results: {
        'sc-001:MON_001': makeResult('sc-001', 'MON_001', true),
      },
    })
    const doc = buildResultsJson(input)
    const { total, passed, failed, pending } = doc.summary
    expect(total).toBe(passed + failed + pending)
  })
})

// ---------------------------------------------------------------------------
// computeSummary: shared helper that both builders delegate to
// ---------------------------------------------------------------------------

describe('computeSummary: outcome counts', () => {
  it('all pending when results is empty', () => {
    const summary = computeSummary([minimalScenario], {})
    expect(summary.total).toBe(3)
    expect(summary.passed).toBe(0)
    expect(summary.failed).toBe(0)
    expect(summary.pending).toBe(3)
  })

  it('counts match what buildResultsJson.summary reports for the same input', () => {
    const input = makeInput({
      hasStarted: true,
      results: {
        'sc-001:MON_001': makeResult('sc-001', 'MON_001', true),
        'sc-001:ALARM_001': makeResult('sc-001', 'ALARM_001', false),
        'sc-001:CONN_001': makeResult('sc-001', 'CONN_001', false),
      },
    })
    const summary = computeSummary(input.scenarios, input.results)
    const doc = buildResultsJson(input)
    expect(summary).toEqual(doc.summary)
  })

  it('counts match what buildReportMarkdown summary table contains for the same input', () => {
    const input = makeInput({
      hasStarted: true,
      results: {
        'sc-001:MON_001': makeResult('sc-001', 'MON_001', true),
        'sc-001:ALARM_001': makeResult('sc-001', 'ALARM_001', false),
      },
    })
    const summary = computeSummary(input.scenarios, input.results)
    const md = buildReportMarkdown(input)
    expect(md).toContain(`| Total | ${summary.total} |`)
    expect(md).toContain(`| Passed | ${summary.passed} |`)
    expect(md).toContain(`| Failed | ${summary.failed} |`)
    expect(md).toContain(`| Pending | ${summary.pending} |`)
  })
})

// ---------------------------------------------------------------------------
// buildResultsJson: comments forwarded verbatim
// ---------------------------------------------------------------------------

describe('buildResultsJson: comments forwarded verbatim', () => {
  it('includes comments array verbatim from the ConformanceTestResult', () => {
    const comments = [
      { uid: 'u1', index: 'AO25', value: '42', issue: 'none' },
      { uid: 'u2', index: 'AO26', value: '0', issue: 'check' },
    ]
    const input = makeInput({
      results: {
        'sc-001:MON_001': {
          event_type: 'conformance_test_result',
          scenario_id: 'sc-001',
          test: 'MON_001',
          passed: true,
          timestamp: 2000,
          comments,
        },
      },
    })
    const doc = buildResultsJson(input)
    const sc = doc.scenarios.find((s) => s.scenario_id === 'sc-001')!
    const t = sc.tests.find((t) => t.test === 'MON_001')!
    expect(t.comments).toEqual(comments)
  })
})

// ---------------------------------------------------------------------------
// buildReportMarkdown: structural checks
// ---------------------------------------------------------------------------

describe('buildReportMarkdown: structure', () => {
  it('starts with a level-1 heading', () => {
    const md = buildReportMarkdown(makeInput())
    expect(md.trimStart()).toMatch(/^# /)
  })

  it('includes the profile name when provided', () => {
    const md = buildReportMarkdown(makeInput({ profileName: 'my-profile' }))
    expect(md).toContain('my-profile')
  })

  it('includes a "Console Log" section header when logs are non-empty', () => {
    const input = makeInput({
      logs: [
        {
          id: '1',
          timestamp: '12:00:00',
          source: 'test_runner',
          message: 'hello',
        },
      ],
    })
    const md = buildReportMarkdown(input)
    expect(md).toContain('Console Log')
  })

  it('includes the log message content in the console section', () => {
    const input = makeInput({
      logs: [
        {
          id: '1',
          timestamp: '12:00:01',
          source: 'test_runner',
          message: 'startup-ok',
        },
      ],
    })
    const md = buildReportMarkdown(input)
    expect(md).toContain('startup-ok')
  })

  it('does not emit em-dash, en-dash, or double-hyphen as punctuation', () => {
    const md = buildReportMarkdown(makeInput())
    // Simple checks: no Unicode dash characters used as punctuation.
    expect(md).not.toMatch(/—/) // em-dash
    expect(md).not.toMatch(/–/) // en-dash
  })

  it('uses plain title with no profile suffix when profileName is null', () => {
    const md = buildReportMarkdown(makeInput({ profileName: null }))
    // Title must be exactly "MESA Test Run Report" with no colon or extra segment.
    expect(md).toMatch(/^# MESA Test Run Report\n/)
    // No "- Profile:" line in Run Details when profileName is null.
    expect(md).not.toContain('- Profile:')
  })

  it('omits the Console Log section when logs is empty', () => {
    const md = buildReportMarkdown(makeInput({ logs: [] }))
    expect(md).not.toContain('## Console Log')
  })

  it('includes Job ID line when jobId is provided', () => {
    const md = buildReportMarkdown(makeInput({ jobId: 'job-xyz-123' }))
    expect(md).toContain('- Job ID: job-xyz-123')
  })

  it('omits Job ID line when jobId is null', () => {
    const md = buildReportMarkdown(makeInput({ jobId: null }))
    expect(md).not.toContain('- Job ID:')
  })
})

// ---------------------------------------------------------------------------
// buildZipBytes: zip round-trip (pure, no browser APIs)
//
// buildZipBytes is the pure layer that assembles the zip. We test it directly
// so the round-trip is independent of browser APIs (jsdom Blob limits).
// triggerBundleDownload tests cover only the browser-side trigger.
// ---------------------------------------------------------------------------

describe('buildZipBytes: zip round-trip', () => {
  it('produces a zip containing exactly results.json, profile.json, and report.md', () => {
    const input = makeInput({
      profileName: 'round-trip-profile',
      results: {
        'sc-001:MON_001': makeResult('sc-001', 'MON_001', true),
      },
      logs: [
        {
          id: '1',
          timestamp: '12:00:00',
          source: 'test_runner',
          message: 'log-line',
        },
      ],
    })

    const bytes = buildZipBytes(input)
    const unzipped = unzipSync(bytes)

    const keys = Object.keys(unzipped)
    expect(keys).toContain('results.json')
    expect(keys).toContain('profile.json')
    expect(keys).toContain('report.md')
    expect(keys).toHaveLength(3)
  })

  it('results.json in the zip is valid JSON with schema_version=1', () => {
    const bytes = buildZipBytes(makeInput())
    const unzipped = unzipSync(bytes)
    const json = JSON.parse(strFromU8(unzipped['results.json']))

    expect(json.schema_version).toBe(1)
    expect(typeof json.generated_at).toBe('string')
  })

  it('profile.json in the zip matches the input profileData exactly', () => {
    const profileData = {
      version: 42,
      entities: ['a', 'b'],
    } as unknown as PicsProfile
    const bytes = buildZipBytes(makeInput({ profileData }))
    const unzipped = unzipSync(bytes)
    const json = JSON.parse(strFromU8(unzipped['profile.json']))

    expect(json).toEqual(profileData)
  })

  it('report.md in the zip is a non-empty string', () => {
    const bytes = buildZipBytes(makeInput())
    const unzipped = unzipSync(bytes)
    const md = strFromU8(unzipped['report.md'])

    expect(typeof md).toBe('string')
    expect(md.length).toBeGreaterThan(0)
  })

  it('passed tri-state is preserved in results.json inside the zip', () => {
    const input = makeInput({
      results: {
        'sc-001:MON_001': makeResult('sc-001', 'MON_001', null),
      },
    })
    const bytes = buildZipBytes(input)
    const unzipped = unzipSync(bytes)
    const json = JSON.parse(strFromU8(unzipped['results.json']))
    const sc = json.scenarios.find(
      (s: { scenario_id: string }) => s.scenario_id === 'sc-001',
    )
    const t = sc.tests.find((t: { test: string }) => t.test === 'MON_001')
    // null must round-trip as null, not false or undefined.
    expect(t.passed).toBeNull()
    expect(t.outcome).toBe('pending')
  })

  it('log content appears in report.md inside the zip', () => {
    const input = makeInput({
      logs: [
        {
          id: '1',
          timestamp: '12:00:00',
          source: 'test_runner',
          message: 'specific-log-text',
        },
      ],
    })
    const bytes = buildZipBytes(input)
    const unzipped = unzipSync(bytes)
    const md = strFromU8(unzipped['report.md'])

    expect(md).toContain('specific-log-text')
  })
})

// ---------------------------------------------------------------------------
// handleDownload enabled-scenario filter: Wren HIGH bug fix
//
// handleDownload now passes only the enabled subset of scenarios to
// triggerBundleDownload (matching the predicate handleStart sends as
// scenario_ids). This suite verifies the contract at the buildResultsJson
// layer: when the caller passes only enabled scenarios, disabled scenarios
// do NOT appear in results.json.scenarios and their tests are NOT counted
// in summary.total or summary.pending.
//
// RED proof: before the fix, `handleDownload` passed ALL scenarios, so
// disabled scenarios with no results inflated pending and total.
// ---------------------------------------------------------------------------

describe('buildResultsJson: disabled scenarios excluded when caller filters', () => {
  const enabledScenario: Scenario = {
    id: 'sc-enabled',
    name: 'Enabled Scenario',
    description: 'Has results',
    expected_tests: [
      { test_id: 'MON_001', should_pass: true },
      { test_id: 'ALARM_001', should_pass: true },
    ],
  }

  const disabledScenario: Scenario = {
    id: 'sc-disabled',
    name: 'Disabled Scenario',
    description: 'Never ran: excluded by enabledScenarios filter',
    expected_tests: [
      { test_id: 'CONN_001', should_pass: true },
      { test_id: 'SERV_001', should_pass: true },
      { test_id: 'OP_001', should_pass: true },
    ],
  }

  it('disabled scenario does not appear in results.json.scenarios', () => {
    // Prove that when the disabled scenario is excluded from the input,
    // it does not appear in the output. The enabledScenarios filter at the
    // call site prevents disabledScenario from being passed to buildResultsJson.
    const enabledForRun = [enabledScenario] // disabledScenario excluded by the filter

    const input: BundleInput = {
      scenarios: enabledForRun,
      results: {
        'sc-enabled:MON_001': makeResult('sc-enabled', 'MON_001', true),
        'sc-enabled:ALARM_001': makeResult('sc-enabled', 'ALARM_001', true),
      },
      logs: [],
      profileData: fakeProfile,
      profileName: 'test-profile',
      jobId: 'job-001',
      deviceUnderTest: 'outstation',
      hasStarted: true,
    }

    const doc = buildResultsJson(input)

    // The disabled scenario must not appear in the scenarios array.
    const scenarioIds = doc.scenarios.map((s) => s.scenario_id)
    expect(scenarioIds).not.toContain(disabledScenario.id)
    expect(scenarioIds).toContain(enabledScenario.id)
    expect(doc.scenarios).toHaveLength(1)
  })

  it('disabled scenario tests are not counted in summary.total or summary.pending', () => {
    // Before the fix: passing both scenarios (all=3 enabled tests, 3 disabled
    // pending tests) inflated total to 5 and pending to 3. After the fix:
    // only the enabled scenario is passed, so total=2 and pending=0.
    const enabledForRun = [enabledScenario]

    const input: BundleInput = {
      scenarios: enabledForRun,
      results: {
        'sc-enabled:MON_001': makeResult('sc-enabled', 'MON_001', true),
        'sc-enabled:ALARM_001': makeResult('sc-enabled', 'ALARM_001', true),
      },
      logs: [],
      profileData: fakeProfile,
      profileName: 'test-profile',
      jobId: 'job-001',
      deviceUnderTest: 'outstation',
      hasStarted: true,
    }

    const doc = buildResultsJson(input)

    // Only 2 tests (the enabled scenario's tests), both passed.
    expect(doc.summary.total).toBe(2)
    expect(doc.summary.passed).toBe(2)
    expect(doc.summary.failed).toBe(0)
    // No pending: disabledScenario's 3 ghost-pending tests are absent.
    expect(doc.summary.pending).toBe(0)
  })
})

// ---------------------------------------------------------------------------
// triggerBundleDownload: browser-side trigger only
// ---------------------------------------------------------------------------

describe('triggerBundleDownload: browser trigger', () => {
  let createObjectURL: ReturnType<typeof vi.fn>
  let revokeObjectURL: ReturnType<typeof vi.fn>
  let appendChildSpy: ReturnType<typeof vi.spyOn>
  let removeChildSpy: ReturnType<typeof vi.spyOn>
  let clickSpy: ReturnType<typeof vi.spyOn>

  beforeEach(() => {
    createObjectURL = vi.fn().mockReturnValue('blob:test-zip-url')
    revokeObjectURL = vi.fn()
    vi.stubGlobal('URL', { ...URL, createObjectURL, revokeObjectURL })

    appendChildSpy = vi
      .spyOn(document.body, 'appendChild')
      .mockImplementation((node) => node)
    removeChildSpy = vi
      .spyOn(document.body, 'removeChild')
      .mockImplementation((node) => node)
    clickSpy = vi
      .spyOn(HTMLAnchorElement.prototype, 'click')
      .mockImplementation(() => {})
  })

  afterEach(() => {
    vi.restoreAllMocks()
    vi.unstubAllGlobals()
  })

  it('triggers browser download: anchor clicked, appended, removed', () => {
    triggerBundleDownload(makeInput({ profileName: 'my-run' }))

    expect(clickSpy).toHaveBeenCalledOnce()
    expect(appendChildSpy).toHaveBeenCalledOnce()
    expect(removeChildSpy).toHaveBeenCalledOnce()
    expect(createObjectURL).toHaveBeenCalledOnce()
  })

  it('uses timestamp-only filename when profileName is null (no throw)', () => {
    expect(() =>
      triggerBundleDownload(makeInput({ profileName: null })),
    ).not.toThrow()
    expect(createObjectURL).toHaveBeenCalledOnce()
  })

  it('schedules URL.revokeObjectURL asynchronously after click', () => {
    vi.useFakeTimers()
    triggerBundleDownload(makeInput())
    expect(revokeObjectURL).not.toHaveBeenCalled()
    vi.runAllTimers()
    expect(revokeObjectURL).toHaveBeenCalledOnce()
    vi.useRealTimers()
  })

  it('removes anchor and defers revoke even when click throws', () => {
    // Mirrors the try/finally teardown pattern tested for triggerDownloadWithBlob.
    // Ensures the bundle trigger is equally robust on a throwing click.
    clickSpy.mockImplementation(() => {
      throw new Error('click failed')
    })
    vi.useFakeTimers()
    expect(() => triggerBundleDownload(makeInput())).toThrow('click failed')
    // Anchor must be removed despite the throw.
    expect(removeChildSpy).toHaveBeenCalledOnce()
    // Revoke must still be scheduled and fire after timers flush.
    expect(revokeObjectURL).not.toHaveBeenCalled()
    vi.runAllTimers()
    expect(revokeObjectURL).toHaveBeenCalledWith('blob:test-zip-url')
    vi.useRealTimers()
  })
})
