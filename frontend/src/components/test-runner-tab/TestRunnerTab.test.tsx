/**
 * Component-level tests for the log Copy and Download actions in TestRunnerTab.
 *
 * Focus: scope (filteredLogs), disabled guards, and DOM-side-effect isolation.
 * We do NOT test SSE wiring, handleStart/handleStop, or scenario selection here —
 * except in the filter-scope and toast-error tests, which use a MockEventSource
 * to inject log entries into the component via the SSE path so that behavioral
 * assertions can be made against real state.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import {
  render,
  screen,
  waitFor,
  fireEvent,
  act,
  within,
} from '@testing-library/react'
import TestRunnerTab from './TestRunnerTab'
import PointResultsTable from './PointResultsTable'
import * as downloadLogs from './downloadLogs'
import * as exportBundle from './exportBundle'
import * as sonner from 'sonner'
import type { PicsProfile } from '@/api/generated'
import fullProfileJson from '../../../../data/profiles/full.json'
import type { Preferences } from '../../utils/preferences'
import type { LogSource, ProcessStatus, SunSpecComment } from './types'
import { fetchScenarios } from './scenarios'

// ---------------------------------------------------------------------------
// MockEventSource: injectable SSE stand-in
//
// The component creates a new EventSource inside handleStart. By stubbing the
// global EventSource constructor we capture the instance and can fire open/
// message events synchronously from test code.
// ---------------------------------------------------------------------------

class MockEventSource {
  url: string
  readyState: number
  onopen: ((event: Event) => void) | null = null
  onmessage: ((event: { data: string }) => void) | null = null
  onerror: ((event: Event) => void) | null = null
  static CONNECTING = 0
  static OPEN = 1
  static CLOSED = 2
  static instance: MockEventSource | null = null

  constructor(url: string) {
    this.url = url
    this.readyState = MockEventSource.CONNECTING
    MockEventSource.instance = this
  }

  close() {
    this.readyState = MockEventSource.CLOSED
  }

  simulateOpen() {
    this.readyState = MockEventSource.OPEN
    if (this.onopen) this.onopen(new Event('open'))
  }

  simulateMessage(source: LogSource, message: string) {
    if (this.onmessage) {
      this.onmessage({
        data: JSON.stringify({
          source,
          event_type: 'log',
          message: { event_type: 'log', message },
        }),
      })
    }
  }

  simulateConformanceResult(
    scenarioId: string,
    testId: string,
    passed: boolean,
    comments: SunSpecComment[] = [],
  ) {
    if (this.onmessage) {
      // The SSE envelope wraps the conformance object in the `message` field,
      // matching the real backend format that handleEvent -> dispatchTestRunnerEvent
      // unpacks via the `{ source, event_type, message }` destructure.
      const conformanceObj = {
        event_type: 'conformance_test_result',
        test: testId,
        scenario_id: scenarioId,
        passed,
        timestamp: Date.now(),
        comments,
      }
      this.onmessage({
        data: JSON.stringify({
          source: 'test_runner',
          event_type: 'conformance_test_result',
          message: conformanceObj,
        }),
      })
    }
  }

  simulateStatus(status: ProcessStatus) {
    if (this.onmessage) {
      this.onmessage({
        data: JSON.stringify({
          source: 'test_runner',
          event_type: 'status_update',
          message: {
            event_type: 'status_update',
            process: 'test_runner',
            status,
          },
        }),
      })
    }
  }

  static reset() {
    MockEventSource.instance = null
  }
}

// ---------------------------------------------------------------------------
// Minimal prop stubs
// ---------------------------------------------------------------------------

const fakeProfile = {
  AI: {
    points: [{ point_index: 12, name: 'Test point' }],
    meters: [],
    ders: [],
    inverters: [],
    batteries: [],
    curves: [],
    schedules: [],
    schedules_bc: [],
  },
  BI: { points: [], meters: [], ders: [], inverters: [], batteries: [] },
  AO: { points: [], meters: [], inverters: [], batteries: [] },
  BO: { points: [] },
} as unknown as PicsProfile

const fakePreferences: Preferences = {
  outstationIp: '127.0.0.1',
  outstationPort: 20000,
  enabledScenarios: {},
  deviceUnderTest: 'outstation',
  autoExpandSections: false,
}

// ---------------------------------------------------------------------------
// Helpers: inject log entries via the SSE addLog path is complex; instead we
// use the component's internal event system via CustomEvent, matching how
// eventDispatch.ts fires into the component. But that's deep plumbing.
//
// Simpler approach: we spy on `formatLogsForDownload` and `triggerBundleDownload`
// from the respective modules so we can assert which logs were passed and that
// the trigger fires with the correct serialized text.
//
// To get logs into the component state we trigger the internal `addLog` by
// firing the component's SSE-connected `handleStart` flow — which is too heavy
// for unit tests. Instead, we test the disabled guard (empty filteredLogs)
// by checking button disabled state, and test the scope separately via the
// module-level unit tests in downloadLogs.test.ts.
//
// For the "only filtered logs" invariant, we stub `fetchScenarios` to avoid
// network calls and verify button disable behavior directly.
// ---------------------------------------------------------------------------

vi.mock('./scenarios', () => ({
  fetchScenarios: vi.fn().mockResolvedValue([]),
}))

// Mock URL APIs that jsdom does not implement.
// Spread the real URL so other URL members (e.g. URL constructor, parse, etc.) survive.
const createObjectURLMock = vi.fn().mockReturnValue('blob:test-url')
const revokeObjectURLMock = vi.fn()

beforeEach(() => {
  vi.stubGlobal('URL', {
    ...URL,
    createObjectURL: createObjectURLMock,
    revokeObjectURL: revokeObjectURLMock,
  })
})

afterEach(() => {
  vi.restoreAllMocks()
  vi.unstubAllGlobals()
})

describe('TestRunnerTab log action buttons', () => {
  it('Copy Logs button is disabled when there are no logs', async () => {
    render(
      <TestRunnerTab profileData={fakeProfile} preferences={fakePreferences} />,
    )
    // Wait for scenarios fetch to settle
    await waitFor(() => {
      const btn = screen.getByRole('button', { name: /copy logs/i })
      expect(btn).toBeDisabled()
    })
  })

  it('Download button is disabled when there are no test results', async () => {
    render(
      <TestRunnerTab profileData={fakeProfile} preferences={fakePreferences} />,
    )
    await waitFor(() => {
      const btn = screen.getByRole('button', { name: /^download$/i })
      expect(btn).toBeDisabled()
    })
  })

  it('Clear Logs button is disabled when there are no logs', async () => {
    render(
      <TestRunnerTab profileData={fakeProfile} preferences={fakePreferences} />,
    )
    await waitFor(() => {
      const btn = screen.getByRole('button', { name: /clear logs/i })
      expect(btn).toBeDisabled()
    })
  })

  it('Download button exists and is accessible (label "Download")', async () => {
    render(
      <TestRunnerTab profileData={fakeProfile} preferences={fakePreferences} />,
    )
    await waitFor(() => {
      expect(
        screen.getByRole('button', { name: /^download$/i }),
      ).toBeInTheDocument()
    })
  })
})

// ---------------------------------------------------------------------------
// Empty-filteredLogs guard
// These tests verify that clicking the button while filteredLogs is logically
// empty (even if React state flush is momentarily async) never calls the
// download/clipboard path and never fires a success toast.
// ---------------------------------------------------------------------------

describe('handleDownload / handleCopyLogs empty-guard', () => {
  it('handleDownload does not call triggerBundleDownload when testResults is empty', async () => {
    const triggerSpy = vi
      .spyOn(exportBundle, 'triggerBundleDownload')
      .mockImplementation(() => {})

    render(
      <TestRunnerTab profileData={fakeProfile} preferences={fakePreferences} />,
    )

    await waitFor(() => {
      const btn = screen.getByRole('button', { name: /^download$/i })
      // Button is disabled when no test results: clicking must not invoke the download path.
      expect(btn).toBeDisabled()
    })

    const btn = screen.getByRole('button', { name: /^download$/i })
    // Force-fire click to simulate a race that slips past the disabled attribute.
    fireEvent.click(btn)

    expect(triggerSpy).not.toHaveBeenCalled()
    triggerSpy.mockRestore()
  })

  it('handleCopyLogs does not call clipboard.writeText when filteredLogs is empty', async () => {
    const writeTextMock = vi.fn().mockResolvedValue(undefined)
    Object.defineProperty(navigator, 'clipboard', {
      value: { writeText: writeTextMock },
      configurable: true,
    })

    render(
      <TestRunnerTab profileData={fakeProfile} preferences={fakePreferences} />,
    )

    await waitFor(() => {
      const btn = screen.getByRole('button', { name: /copy logs/i })
      expect(btn).toBeDisabled()
    })

    const btn = screen.getByRole('button', { name: /copy logs/i })
    fireEvent.click(btn)

    expect(writeTextMock).not.toHaveBeenCalled()
  })
})

// ---------------------------------------------------------------------------
// Helpers for SSE-injection tests
//
// renderWithLogs: renders the component, stubs fetch and EventSource, clicks
// Start, fires the SSE open event and then one simulated log message per
// entry in `entries`. Returns the MockEventSource instance so callers can
// fire additional messages.
// ---------------------------------------------------------------------------

interface LogSpec {
  source: LogSource
  message: string
}

// Preferences variant with one enabled scenario, so handleStart passes the
// "at least one scenario enabled" guard and proceeds to create an EventSource.
const fakePreferencesWithScenario: Preferences = {
  ...fakePreferences,
  enabledScenarios: { 'sc-001': true },
}

async function renderWithLogs(entries: LogSpec[]): Promise<MockEventSource> {
  // Stub fetch so the POST /api/jobs call returns a job_id immediately.
  vi.stubGlobal(
    'fetch',
    vi.fn().mockResolvedValue({
      ok: true,
      json: async () => ({ job_id: 'test-job-001' }),
    }),
  )

  // Stub the EventSource constructor.
  vi.stubGlobal('EventSource', MockEventSource)

  render(
    <TestRunnerTab
      profileData={fakeProfile}
      preferences={fakePreferencesWithScenario}
    />,
  )

  // Wait for the initial render to settle (scenarios fetch resolves to []).
  await waitFor(() => screen.getByRole('button', { name: /start test run/i }))

  // Click Start to trigger handleStart, which calls fetch then creates EventSource.
  await act(async () => {
    fireEvent.click(screen.getByRole('button', { name: /start test run/i }))
  })

  // handleStart is async (fetch + state updates); wait for it to finish
  // by waiting for the EventSource instance to exist.
  await waitFor(() => {
    if (!MockEventSource.instance)
      throw new Error('EventSource not created yet')
  })

  const es = MockEventSource.instance!

  // Fire onopen so the component sets process status and adds the "Connected" web_client log.
  await act(async () => {
    es.simulateOpen()
  })

  // Inject the caller-supplied log entries as SSE messages.
  for (const entry of entries) {
    await act(async () => {
      es.simulateMessage(entry.source, entry.message)
    })
  }

  return es
}

// ---------------------------------------------------------------------------
// handleDownload filename stamp (zip format)
// ---------------------------------------------------------------------------

describe('handleDownload filename stamp', () => {
  it('zip filename contains no colons or dots in the stamp segment', () => {
    const stamp = new Date('2024-01-15T10:30:45.123Z')
      .toISOString()
      .replace(/[:.]/g, '-')
    const filename = `mesa-test-run-my-profile-${stamp}.zip`
    expect(filename).toBe(
      'mesa-test-run-my-profile-2024-01-15T10-30-45-123Z.zip',
    )
    const stampSegment = filename
      .replace(/^mesa-test-run-my-profile-/, '')
      .replace(/\.zip$/, '')
    expect(stampSegment).not.toContain(':')
    expect(stampSegment).not.toContain('.')
  })
})

// ---------------------------------------------------------------------------
// Download button enabled on testResults non-empty
//
// The Download button is gated on Object.keys(testResults).length > 0, not on
// filteredLogs. Inject a conformance_test_result event and verify it becomes
// enabled.
// ---------------------------------------------------------------------------

describe('handleDownload enabled when testResults is non-empty', () => {
  afterEach(() => {
    vi.restoreAllMocks()
    vi.unstubAllGlobals()
    MockEventSource.reset()
  })

  it('Download button becomes enabled after receiving a conformance result', async () => {
    // Stub triggerBundleDownload to prevent real DOM side-effects.
    vi.spyOn(exportBundle, 'triggerBundleDownload').mockImplementation(() => {})

    const es = await renderWithLogs([])

    // Confirm Download starts disabled (no test results yet).
    await waitFor(() => {
      expect(screen.getByRole('button', { name: /^download$/i })).toBeDisabled()
    })

    // Inject a conformance_test_result via the SSE path.
    await act(async () => {
      es.simulateConformanceResult('sc-001', 'MON_001', true)
    })

    // Download must now be enabled.
    await waitFor(() => {
      expect(
        screen.getByRole('button', { name: /^download$/i }),
      ).not.toBeDisabled()
    })
  })
})

describe('point results', () => {
  afterEach(() => MockEventSource.reset())

  it('shows readback checks only for applicable curve and schedule points', () => {
    const fullProfile = fullProfileJson as PicsProfile
    const curve = fullProfile.AI.curves[0]
    const schedule = fullProfile.AI.schedules[0]
    const scheduleBc = fullProfile.AI.schedules_bc[0]
    const profile = {
      ...fakeProfile,
      AI: {
        ...fakeProfile.AI,
        curves: [
          {
            ...curve,
            curve_type: {
              ...curve.curve_type,
              point_index: 60001,
              assoc_ao: 'AO60002',
            },
            x_values: [
              { ...curve.curve_type, point_index: 60009, assoc_ao: null },
            ],
            y_values: [
              { ...curve.curve_type, point_index: 60010, assoc_ao: null },
            ],
          },
        ],
        schedules: [
          {
            ...schedule,
            identity: {
              ...schedule.identity,
              point_index: 60003,
              assoc_ao: 'AO60004',
            },
            status: { ...schedule.status, point_index: 60005, assoc_ao: null },
            time_offsets: [
              { ...schedule.identity, point_index: 60011, assoc_ao: 'AO60011' },
            ],
            action_types: [],
            action_indexes: [],
            values: [],
          },
        ],
        schedules_bc: [
          {
            ...scheduleBc,
            identity: {
              ...scheduleBc.identity,
              point_index: 60006,
              assoc_ao: 'AO60007',
            },
            status: {
              ...scheduleBc.status,
              point_index: 60008,
              assoc_ao: null,
            },
            time_offsets: [
              {
                ...scheduleBc.identity,
                point_index: 60012,
                assoc_ao: 'AO60012',
              },
            ],
            values: [],
          },
        ],
      },
      AO: {
        ...fakeProfile.AO,
        points: [
          {
            ...fullProfile.AO.points[0],
            point_index: 60002,
            assoc_ai: 'AI60001',
          },
          {
            ...fullProfile.AO.points[0],
            point_index: 60004,
            assoc_ai: 'AI60003',
          },
        ],
      },
      BI: {
        ...fakeProfile.BI,
        points: [{ ...fullProfile.BI.points[0], point_index: 7 }],
      },
      BO: {
        ...fakeProfile.BO,
        points: [{ ...fullProfile.BO.points[0], point_index: 8 }],
      },
    } as PicsProfile
    const comment: SunSpecComment = {
      uid: 'uid',
      index: 'AI60001',
      check: 'curve_readback_matches_profile',
      value: '',
      message: 'Curve matches',
      passed: true,
    }
    render(
      <PointResultsTable
        profile={profile}
        results={{
          test: {
            event_type: 'conformance_test_result',
            scenario_id: 'scenario',
            test: 'MON_001',
            timestamp: 1,
            passed: false,
            comments: [
              comment,
              {
                ...comment,
                index: 'AI60003',
                check: 'schedule_readback_matches_profile',
                message: 'Schedule differs',
                passed: false,
              },
              { ...comment, index: 'AI12', passed: false },
            ],
          },
        }}
      />,
    )

    expect(
      screen.getByLabelText(
        'AO60002 / AI60001, Curve readback matches profile: Passed. AI60001: Curve matches',
        { selector: 'td' },
      ),
    ).toBeInTheDocument()
    expect(
      screen.getByLabelText(
        'AO60004 / AI60003, Schedule readback matches profile: Failed. AI60003: Schedule differs',
        { selector: 'td' },
      ),
    ).toBeInTheDocument()
    expect(
      screen.getByLabelText(
        'AI60006, Schedule readback matches profile: No information',
        { selector: 'td' },
      ),
    ).toBeInTheDocument()
    for (const index of ['AI60009', 'AI60010']) {
      expect(
        screen.getByLabelText(
          `${index}, Curve readback matches profile: No information`,
          { selector: 'td' },
        ),
      ).toBeInTheDocument()
    }
    for (const index of ['AI60011', 'AI60012']) {
      expect(
        screen.getByLabelText(
          `${index}, Schedule readback matches profile: No information`,
          { selector: 'td' },
        ),
      ).toBeInTheDocument()
    }
    for (const index of ['AI12', 'BI7', 'BO8', 'AI60005', 'AI60008']) {
      for (const label of [
        'Curve readback matches profile',
        'Schedule readback matches profile',
      ]) {
        expect(
          screen.getByLabelText(`${index}, ${label}: Not applicable`, {
            selector: 'td',
          }),
        ).toHaveTextContent('n/a')
      }
    }
    expect(
      screen.getByLabelText(
        'AO60002 / AI60001, Schedule readback matches profile: Not applicable',
        { selector: 'td' },
      ),
    ).toHaveTextContent('n/a')
    expect(
      screen.getByLabelText(
        'AO60004 / AI60003, Curve readback matches profile: Not applicable',
        { selector: 'td' },
      ),
    ).toHaveTextContent('n/a')

    fireEvent.click(
      screen.getByRole('button', {
        name: 'Sort by Curve readback matches profile',
      }),
    )
    const sortedRows = screen.getAllByRole('rowheader')
    expect(sortedRows[sortedRows.length - 1]).toHaveTextContent(
      'AO60002 / AI60001',
    )
    fireEvent.click(
      screen.getByRole('button', {
        name: 'Sort by Schedule readback matches profile',
      }),
    )
    expect(screen.getAllByRole('rowheader')[0]).toHaveTextContent(
      'AO60004 / AI60003',
    )
  })

  it('shows n/a for range checks on binary points and pairs', () => {
    const profile = {
      ...fakeProfile,
      AI: { ...fakeProfile.AI, points: [{ point_index: 12 }] },
      AO: { ...fakeProfile.AO, points: [{ point_index: 5 }] },
      BI: {
        ...fakeProfile.BI,
        points: [{ point_index: 7 }, { point_index: 8 }],
      },
      BO: {
        ...fakeProfile.BO,
        points: [{ point_index: 9, assoc_bi: 'BI8' }, { point_index: 10 }],
      },
    } as PicsProfile
    render(
      <PointResultsTable
        profile={profile}
        results={{
          test: {
            event_type: 'conformance_test_result',
            scenario_id: 'scenario',
            test: 'MON_001',
            timestamp: 1,
            passed: false,
            comments: [
              {
                uid: 'uid',
                index: 'BI7',
                check: 'within_profile_range',
                value: '',
                message: 'Unexpected binary range result',
                passed: false,
              },
            ],
          },
        }}
      />,
    )

    for (const index of ['BI7', 'BO10', 'BO9 / BI8']) {
      expect(
        screen.getByRole('cell', {
          name: `${index}, Within profile range: Not applicable`,
        }),
      ).toHaveTextContent('n/a')
    }
    for (const index of ['AI12', 'AO5']) {
      expect(
        screen.getByRole('cell', {
          name: `${index}, Within profile range: No information`,
        }),
      ).toBeInTheDocument()
    }
  })

  it('shows n/a for writes to input points without an associated output', () => {
    const profile = {
      ...fakeProfile,
      AI: {
        ...fakeProfile.AI,
        points: [
          { point_index: 12, name: 'Input only' },
          { point_index: 13, name: 'Paired input' },
        ],
      },
      AO: {
        ...fakeProfile.AO,
        points: [{ point_index: 5, assoc_ai: 'AI13' }, { point_index: 6 }],
      },
      BI: {
        ...fakeProfile.BI,
        points: [
          { point_index: 7, name: 'Binary input only' },
          { point_index: 8, name: 'Paired binary input' },
        ],
      },
      BO: { ...fakeProfile.BO, points: [{ point_index: 9, assoc_bi: 'BI8' }] },
    } as PicsProfile
    render(
      <PointResultsTable
        profile={profile}
        results={{
          test: {
            event_type: 'conformance_test_result',
            scenario_id: 'scenario',
            test: 'MON_001',
            timestamp: 1,
            passed: false,
            comments: [
              {
                uid: 'uid',
                index: 'AI12',
                check: 'written_to_outstation',
                value: '',
                message: 'Not written',
                passed: false,
              },
            ],
          },
        }}
      />,
    )

    expect(
      screen.getByRole('cell', {
        name: 'AI12, Written to outstation: Not applicable',
      }),
    ).toHaveTextContent('n/a')
    expect(
      screen.getByRole('cell', {
        name: 'AO5 / AI13, Written to outstation: No information',
      }),
    ).toBeInTheDocument()
    expect(
      screen.getByRole('cell', {
        name: 'AO6, Written to outstation: No information',
      }),
    ).toBeInTheDocument()
    expect(
      screen.getByRole('cell', {
        name: 'BI7, Written to outstation: Not applicable',
      }),
    ).toHaveTextContent('n/a')
    expect(
      screen.getByRole('cell', {
        name: 'BO9 / BI8, Written to outstation: No information',
      }),
    ).toBeInTheDocument()
  })

  it('shows associated AI and AO outcomes on one row', () => {
    const profile = {
      ...fakeProfile,
      AI: {
        ...fakeProfile.AI,
        points: [{ point_index: 12, name: 'Test point' }],
      },
      AO: { ...fakeProfile.AO, points: [{ point_index: 5, assoc_ai: 'AI12' }] },
    } as PicsProfile
    const comment: SunSpecComment = {
      uid: 'uid',
      index: 'AI12',
      check: 'received_from_outstation',
      value: '',
      message: 'Received',
      passed: true,
    }
    render(
      <PointResultsTable
        profile={profile}
        results={{
          test: {
            event_type: 'conformance_test_result',
            scenario_id: 'scenario',
            test: 'MON_001',
            timestamp: 1,
            passed: true,
            comments: [
              comment,
              {
                ...comment,
                index: 'AO5',
                check: 'written_to_outstation',
                message: 'Written',
              },
            ],
          },
        }}
      />,
    )
    expect(screen.getAllByRole('rowheader')).toHaveLength(1)
    expect(
      screen.getByRole('rowheader', { name: 'AO5 / AI12' }),
    ).toBeInTheDocument()
    expect(
      screen.getByRole('cell', {
        name: 'AO5 / AI12, Received from outstation: Passed. AI12: Received',
      }),
    ).toBeInTheDocument()
    expect(
      screen.getByRole('cell', {
        name: 'AO5 / AI12, Written to outstation: Passed. AO5: Written',
      }),
    ).toBeInTheDocument()
  })

  it('sorts point, name, and check columns in both directions', () => {
    const profile = {
      ...fakeProfile,
      AI: {
        ...fakeProfile.AI,
        points: [
          { point_index: 10, name: 'Alpha' },
          { point_index: 2, name: 'Zulu' },
          { point_index: 3, name: 'Middle' },
        ],
      },
    } as PicsProfile
    const comment: SunSpecComment = {
      uid: 'uid',
      index: 'AI2',
      check: 'received_from_outstation',
      value: '',
      message: 'Not received',
      passed: false,
    }
    render(
      <PointResultsTable
        profile={profile}
        results={{
          test: {
            event_type: 'conformance_test_result',
            scenario_id: 'scenario',
            test: 'MON_001',
            timestamp: 1,
            passed: false,
            comments: [
              comment,
              { ...comment, index: 'AI10', passed: true },
              { ...comment, index: 'AI3', check: 'written_to_outstation' },
              {
                ...comment,
                index: 'AI2',
                check: 'written_to_outstation',
                passed: true,
              },
              { ...comment, index: 'AI10', check: 'within_profile_range' },
              {
                ...comment,
                index: 'AI3',
                check: 'within_profile_range',
                passed: true,
              },
              {
                ...comment,
                index: 'AI3',
                check: 'curve_readback_matches_profile',
              },
              {
                ...comment,
                index: 'AI2',
                check: 'curve_readback_matches_profile',
                passed: true,
              },
              {
                ...comment,
                index: 'AI10',
                check: 'schedule_readback_matches_profile',
              },
              {
                ...comment,
                index: 'AI3',
                check: 'schedule_readback_matches_profile',
                passed: true,
              },
            ],
          },
        }}
      />,
    )

    const columnHeaders = within(screen.getByRole('table')).getAllByRole(
      'columnheader',
    )
    expect(columnHeaders[2]).toHaveTextContent('Status')
    expect(columnHeaders[3]).toHaveTextContent('Written to outstation')
    expect(columnHeaders[4]).toHaveTextContent('Received from outstation')

    const indices = () =>
      screen.getAllByRole('rowheader').map((row) => row.textContent)
    const received = screen.getByRole('button', {
      name: 'Sort by Received from outstation',
    })
    fireEvent.click(received)
    expect(indices()).toEqual(['AI2', 'AI3', 'AI10'])
    expect(received.closest('th')).toHaveAttribute('aria-sort', 'ascending')
    fireEvent.click(received)
    expect(indices()).toEqual(['AI10', 'AI3', 'AI2'])
    expect(received.closest('th')).toHaveAttribute('aria-sort', 'descending')

    fireEvent.click(screen.getByRole('button', { name: 'Sort by Name' }))
    expect(indices()).toEqual(['AI10', 'AI3', 'AI2'])
    fireEvent.click(screen.getByRole('button', { name: 'Sort by Name' }))
    expect(indices()).toEqual(['AI2', 'AI3', 'AI10'])
    fireEvent.click(screen.getByRole('button', { name: 'Sort by Point' }))
    expect(indices()).toEqual(['AI2', 'AI3', 'AI10'])
    fireEvent.click(screen.getByRole('button', { name: 'Sort by Point' }))
    expect(indices()).toEqual(['AI10', 'AI3', 'AI2'])
    fireEvent.click(
      screen.getByRole('button', { name: 'Sort by Written to outstation' }),
    )
    expect(indices()).toEqual(['AI2', 'AI3', 'AI10'])
    fireEvent.click(
      screen.getByRole('button', { name: 'Sort by Within profile range' }),
    )
    expect(indices()).toEqual(['AI10', 'AI2', 'AI3'])
    fireEvent.click(
      screen.getByRole('button', {
        name: 'Sort by Curve readback matches profile',
      }),
    )
    expect(indices()).toEqual(['AI2', 'AI3', 'AI10'])
    fireEvent.click(
      screen.getByRole('button', {
        name: 'Sort by Schedule readback matches profile',
      }),
    )
    expect(indices()).toEqual(['AI2', 'AI3', 'AI10'])
    expect(
      within(screen.getByRole('table')).getAllByRole('columnheader'),
    ).toHaveLength(8)
  })

  it('sorts overall status by failed, unknown, and passed checks', () => {
    const profile = {
      ...fakeProfile,
      BI: {
        ...fakeProfile.BI,
        points: [{ point_index: 1 }, { point_index: 2 }, { point_index: 3 }],
      },
    } as PicsProfile
    const comment: SunSpecComment = {
      uid: 'uid',
      index: 'BI1',
      check: 'received_from_outstation',
      value: '',
      message: 'Received',
      passed: true,
    }
    render(
      <PointResultsTable
        profile={profile}
        results={{
          test: {
            event_type: 'conformance_test_result',
            scenario_id: 'scenario',
            test: 'MON_001',
            timestamp: 1,
            passed: false,
            comments: [
              comment,
              {
                ...comment,
                index: 'BI1',
                check: 'within_profile_range',
                passed: false,
              },
              { ...comment, index: 'BI3', passed: false },
              { ...comment, index: 'AI12', passed: false },
            ],
          },
        }}
      />,
    )

    const indices = () =>
      screen.getAllByRole('rowheader').map((row) => row.textContent)
    const status = screen.getByRole('button', { name: 'Sort by Status' })
    expect(status.closest('th')).toHaveAttribute('aria-sort', 'ascending')
    expect(indices()).toEqual(['AI12', 'BI3', 'BI2', 'BI1'])
    expect(
      screen.getByRole('cell', { name: 'AI12, Status: Failed' }),
    ).toBeInTheDocument()
    expect(
      screen.getByRole('cell', { name: 'BI3, Status: Failed' }),
    ).toBeInTheDocument()
    expect(
      screen.getByRole('cell', { name: 'BI2, Status: No information' }),
    ).toBeInTheDocument()
    expect(
      screen.getByRole('cell', { name: 'BI1, Status: Passed' }),
    ).toBeInTheDocument()

    fireEvent.click(status)
    expect(status.closest('th')).toHaveAttribute('aria-sort', 'descending')
    expect(indices()).toEqual(['BI1', 'BI2', 'AI12', 'BI3'])
  })

  it('updates profile rows from conformance events and retains unknown checks', async () => {
    const eventSource = await renderWithLogs([])
    expect(screen.getByRole('rowheader', { name: 'AI12' })).toBeInTheDocument()
    expect(
      screen.getByRole('cell', {
        name: 'AI12, Received from outstation: No information',
      }),
    ).toBeInTheDocument()

    await act(async () => {
      eventSource.simulateConformanceResult('sc-001', 'MON_001', false, [
        {
          uid: 'uid',
          index: 'AI12',
          check: 'received_from_outstation',
          value: 'Not received',
          message: 'AI12 has not been received',
          passed: false,
        },
        {
          uid: 'uid',
          index: 'AI12',
          check: 'within_profile_range',
          value: 'In range',
          message: 'AI12 is within range',
          passed: true,
        },
      ])
    })

    expect(
      screen.getByRole('cell', {
        name: 'AI12, Received from outstation: Failed. AI12 has not been received',
      }),
    ).toHaveAttribute('title', 'AI12 has not been received')
    expect(
      screen.getByRole('cell', {
        name: 'AI12, Within profile range: Passed. AI12 is within range',
      }),
    ).toBeInTheDocument()
    expect(
      screen.getByRole('cell', {
        name: 'AI12, Written to outstation: Not applicable',
      }),
    ).toHaveTextContent('n/a')

    await act(async () => {
      eventSource.simulateConformanceResult('sc-002', 'MON_001', false, [
        {
          uid: 'uid',
          index: 'AI12',
          check: 'within_profile_range',
          value: 'Out of range',
          message: 'AI12 is out of range',
          passed: false,
        },
      ])
    })
    expect(
      screen.getByRole('cell', {
        name: 'AI12, Within profile range: Failed. AI12 is within range. AI12 is out of range',
      }),
    ).toBeInTheDocument()

    fireEvent.click(screen.getByRole('button', { name: /clear logs/i }))
    expect(
      screen.getByRole('cell', {
        name: 'AI12, Received from outstation: Failed. AI12 has not been received',
      }),
    ).toBeInTheDocument()
  })
})

describe('pending test status', () => {
  afterEach(() => {
    vi.restoreAllMocks()
    vi.unstubAllGlobals()
    MockEventSource.reset()
  })

  it('hides pending tests when the runner finishes', async () => {
    vi.mocked(fetchScenarios).mockResolvedValueOnce([
      {
        id: 'sc-001',
        name: 'Scenario One',
        description: 'Scenario with one expected test',
        expected_tests: [{ test_id: 'MON_001', should_pass: true }],
      },
    ])
    vi.stubGlobal(
      'fetch',
      vi.fn().mockResolvedValue({
        ok: true,
        json: async () => ({ job_id: 'test-job-001' }),
      }),
    )
    vi.stubGlobal('EventSource', MockEventSource)

    render(
      <TestRunnerTab
        profileData={fakeProfile}
        preferences={fakePreferencesWithScenario}
      />,
    )

    await screen.findByText('Scenario One')
    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: /start test run/i }))
    })
    await waitFor(() => expect(MockEventSource.instance).not.toBeNull())

    const eventSource = MockEventSource.instance!
    await act(async () => {
      eventSource.simulateStatus('running')
    })
    expect(screen.getByText('1')).toBeInTheDocument()

    await act(async () => {
      eventSource.simulateStatus('finished')
    })
    expect(screen.queryByText('1')).not.toBeInTheDocument()
  })
})

// ---------------------------------------------------------------------------
// Toast-error path: triggerBundleDownload throws => toast.error fires
// ---------------------------------------------------------------------------

describe('handleDownload toast-error path', () => {
  afterEach(() => {
    vi.restoreAllMocks()
    vi.unstubAllGlobals()
    MockEventSource.reset()
  })

  it('fires toast.error with the throw message when triggerBundleDownload throws', async () => {
    // Stub triggerBundleDownload to throw synchronously.
    vi.spyOn(exportBundle, 'triggerBundleDownload').mockImplementation(() => {
      throw new Error('disk full')
    })

    // Spy on the toast methods imported by the component (same sonner module instance).
    const toastErrorSpy = vi
      .spyOn(sonner.toast, 'error')
      .mockImplementation(() => 'mocked-toast-id')
    const toastSuccessSpy = vi
      .spyOn(sonner.toast, 'success')
      .mockImplementation(() => 'mocked-toast-id')

    const es = await renderWithLogs([
      { source: 'test_runner', message: 'test-runner-log-1' },
    ])

    // Inject a conformance result so the Download button becomes enabled.
    await act(async () => {
      es.simulateConformanceResult('sc-001', 'MON_001', true)
    })

    // Wait for the Download button to be enabled.
    await waitFor(() => {
      expect(
        screen.getByRole('button', { name: /^download$/i }),
      ).not.toBeDisabled()
    })

    // Click Download: triggerBundleDownload throws, component catches and calls toast.error.
    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: /^download$/i }))
    })

    // toast.error must have fired with a message containing the error text.
    expect(toastErrorSpy).toHaveBeenCalledOnce()
    const [errorArg] = toastErrorSpy.mock.calls[0]
    expect(typeof errorArg).toBe('string')
    expect(errorArg).toContain('disk full')

    // toast.success must NOT have fired (no successful download occurred).
    expect(toastSuccessSpy).not.toHaveBeenCalled()
  })
})

// ---------------------------------------------------------------------------
// Copy Logs filter-scope guard
//
// handleCopyLogs passes filteredLogs (the on-screen filtered view), not the
// full logs array. Inject entries spanning two sources, activate one filter,
// and assert formatLogsForDownload received only the matching entries.
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// handleCopyLogs toast paths
//
// Success: clipboard.writeText resolves, toast.success fires.
// Error: clipboard.writeText rejects, toast.error fires.
// ---------------------------------------------------------------------------

describe('handleCopyLogs toast paths', () => {
  afterEach(() => {
    vi.restoreAllMocks()
    vi.unstubAllGlobals()
    MockEventSource.reset()
  })

  it('fires toast.success when clipboard.writeText resolves', async () => {
    const writeTextMock = vi.fn().mockResolvedValue(undefined)
    Object.defineProperty(navigator, 'clipboard', {
      value: { writeText: writeTextMock },
      configurable: true,
    })

    const toastSuccessSpy = vi
      .spyOn(sonner.toast, 'success')
      .mockImplementation(() => 'mocked-toast-id')
    const toastErrorSpy = vi
      .spyOn(sonner.toast, 'error')
      .mockImplementation(() => 'mocked-toast-id')

    await renderWithLogs([{ source: 'test_runner', message: 'a-log-entry' }])

    const copyBtn = screen.getByRole('button', { name: /copy logs/i })
    await waitFor(() => expect(copyBtn).not.toBeDisabled())

    await act(async () => {
      fireEvent.click(copyBtn)
    })

    await waitFor(() => {
      expect(toastSuccessSpy).toHaveBeenCalledOnce()
    })
    expect(toastErrorSpy).not.toHaveBeenCalled()
  })

  it('fires toast.error when clipboard.writeText rejects', async () => {
    const writeTextMock = vi
      .fn()
      .mockRejectedValue(new Error('permission denied'))
    Object.defineProperty(navigator, 'clipboard', {
      value: { writeText: writeTextMock },
      configurable: true,
    })

    const toastErrorSpy = vi
      .spyOn(sonner.toast, 'error')
      .mockImplementation(() => 'mocked-toast-id')
    const toastSuccessSpy = vi
      .spyOn(sonner.toast, 'success')
      .mockImplementation(() => 'mocked-toast-id')

    await renderWithLogs([{ source: 'test_runner', message: 'a-log-entry' }])

    const copyBtn = screen.getByRole('button', { name: /copy logs/i })
    await waitFor(() => expect(copyBtn).not.toBeDisabled())

    await act(async () => {
      fireEvent.click(copyBtn)
    })

    await waitFor(() => {
      expect(toastErrorSpy).toHaveBeenCalledOnce()
    })
    const [errorArg] = toastErrorSpy.mock.calls[0]
    expect(typeof errorArg).toBe('string')
    expect(errorArg).toContain('permission denied')
    expect(toastSuccessSpy).not.toHaveBeenCalled()
  })
})

// ---------------------------------------------------------------------------
// Clear Logs preserves results and Download.
// ---------------------------------------------------------------------------

describe('handleClearLogs preserves Download button', () => {
  afterEach(() => {
    vi.restoreAllMocks()
    vi.unstubAllGlobals()
    MockEventSource.reset()
  })

  it('keeps Download enabled after Clear Logs', async () => {
    vi.spyOn(exportBundle, 'triggerBundleDownload').mockImplementation(() => {})

    const es = await renderWithLogs([])

    // Inject a conformance result so Download becomes enabled.
    await act(async () => {
      es.simulateConformanceResult('sc-001', 'MON_001', true)
    })

    await waitFor(() => {
      expect(
        screen.getByRole('button', { name: /^download$/i }),
      ).not.toBeDisabled()
    })

    const clearBtn = screen.getByRole('button', { name: /clear logs/i })
    await act(async () => {
      fireEvent.click(clearBtn)
    })

    await waitFor(() => {
      expect(
        screen.getByRole('button', { name: /^download$/i }),
      ).not.toBeDisabled()
    })
  })
})

// ---------------------------------------------------------------------------

describe('handleCopyLogs filter-scope guard', () => {
  afterEach(() => {
    vi.restoreAllMocks()
    vi.unstubAllGlobals()
    MockEventSource.reset()
  })

  it('passes only the filtered log entries to formatLogsForDownload', async () => {
    const writeTextMock = vi.fn().mockResolvedValue(undefined)
    Object.defineProperty(navigator, 'clipboard', {
      value: { writeText: writeTextMock },
      configurable: true,
    })

    const formatSpy = vi
      .spyOn(downloadLogs, 'formatLogsForDownload')
      .mockReturnValue('formatted')

    await renderWithLogs([
      { source: 'outstation', message: 'outstation-msg-1' },
      { source: 'control_station', message: 'control-msg-1' },
      { source: 'outstation', message: 'outstation-msg-2' },
    ])

    // Activate the control_station log-filter button. Two "control station" buttons exist
    // in the rendered tree: the Device Under Test toggle and the log source filter.
    // The log source filter uses class logSourceControlStation, so select by exact text
    // (lowercase "s" in "station") to distinguish from the DUT toggle ("Control Station").
    const allControlButtons = screen.getAllByRole('button', {
      name: /control station/i,
    })
    // The log filter button has exact text "Control station" (lowercase s); the DUT toggle
    // renders "Control Station" (capital S). Find the filter one.
    const controlFilterBtn = allControlButtons.find(
      (btn) => btn.textContent === 'Control station',
    )!
    fireEvent.click(controlFilterBtn)

    // Click Copy Logs.
    const copyBtn = screen.getByRole('button', { name: /copy logs/i })
    await waitFor(() => expect(copyBtn).not.toBeDisabled())
    fireEvent.click(copyBtn)

    // formatLogsForDownload must have been called with only control_station entries.
    expect(formatSpy).toHaveBeenCalledOnce()
    const [passedLogs] = formatSpy.mock.calls[0]
    // All passed entries must be control_station.
    expect(
      passedLogs.every(
        (l: { source: string }) => l.source === 'control_station',
      ),
    ).toBe(true)
    // No outstation entries must appear.
    expect(
      passedLogs.some((l: { source: string }) => l.source === 'outstation'),
    ).toBe(false)
    // The one control_station entry must be present by message.
    expect(passedLogs[0].message).toBe('control-msg-1')
  })
})
