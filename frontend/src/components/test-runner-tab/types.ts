// Service statuses
export type ProcessName = 'test_runner' | 'control_station' | 'outstation'
export type ProcessStatus =
  | 'unknown'
  | 'starting'
  | 'compiling'
  | 'started'
  | 'running'
  | 'finished'
  | 'error'
  | 'stopped'
  | 'waiting'
  | 'not started'
  | 'stopping'
export type ProcessStatusMap = Map<ProcessName, ProcessStatus>

type TestRunnerStatusMessage = {
  event_type: 'status_update'
  status: ProcessStatus
  process: ProcessName
}

// Logs
export type LogSource = ProcessName | 'web_client'

export type LogEntry = {
  id: string
  timestamp: string
  source: LogSource
  message: string
}

type TestRunnerLogMessage = {
  event_type: 'log'
  message: string
}

// `scenario_id` and `scenario_name` are present on per-scenario events but
// absent on terminal aggregate events (e.g. status `all_complete`). `message`
// is the human-readable description from the backend, optional because not
// every status carries one.
export type TestRunnerScenarioStatusMessage = {
  event_type: 'scenario_status'
  status: string
  scenario_id?: string
  scenario_name?: string
  message?: string
}

/**
 * Represents a single test result entry. Must be kept in sync with backend's ConformanceTestResult model.
 */
export type ConformanceTestResult = {
  event_type: 'conformance_test_result'
  test: string
  scenario_id: string
  timestamp: number
  // `passed` may be absent for seeded/placeholder entries before a real
  // result arrives. Incoming events from the backend will always set a
  // boolean for `passed` (validated by `isConformanceTestResult`).
  passed?: boolean | null
  comments: SunSpecComment[]
}

/**
 * Represents a comment associated with a test result. Must be kept in sync with backend's SunSpecComment model.
 */
export type SunSpecComment = {
  uid: string
  index: string // e.g AO25
  value: string
  issue: string
}

// General events
export type TestRunnerEvent = {
  source: LogSource
  event_type:
    'log' | 'status_update' | 'conformance_test_result' | 'scenario_status'
  message:
    | TestRunnerLogMessage
    | TestRunnerStatusMessage
    | ConformanceTestResult
    | TestRunnerScenarioStatusMessage
}

export function isConformanceTestResult(
  obj: unknown,
): obj is ConformanceTestResult {
  if (typeof obj !== 'object' || obj === null) return false

  const o = obj as Record<string, unknown>
  return (
    o.event_type === 'conformance_test_result' &&
    typeof o.test === 'string' &&
    typeof o.timestamp === 'number' &&
    typeof o.passed === 'boolean' &&
    Array.isArray(o.comments)
  )
}

function isProcessStatus(value: unknown): value is ProcessStatus {
  return (
    value === 'unknown' ||
    value === 'starting' ||
    value === 'compiling' ||
    value === 'started' ||
    value === 'running' ||
    value === 'finished' ||
    value === 'error' ||
    value === 'stopped' ||
    value === 'waiting' ||
    value === 'not started' ||
    value === 'stopping'
  )
}

function isProcessName(value: unknown): value is ProcessName {
  return (
    value === 'test_runner' ||
    value === 'control_station' ||
    value === 'outstation'
  )
}

export function isStatusMessage(obj: unknown): obj is TestRunnerStatusMessage {
  if (typeof obj !== 'object' || obj === null) return false
  const o = obj as Record<string, unknown>
  return (
    o.event_type === 'status_update' &&
    isProcessStatus(o.status) &&
    isProcessName(o.process)
  )
}

export function isScenarioStatusMessage(
  obj: unknown,
): obj is TestRunnerScenarioStatusMessage {
  if (typeof obj !== 'object' || obj === null) return false
  const o = obj as Record<string, unknown>
  // `scenario_id`, `scenario_name`, and `message` are optional on the wire
  // (the terminal `all_complete` envelope omits id/name). Only validate them
  // when present; require `event_type` and `status` always.
  const idOk = o.scenario_id === undefined || typeof o.scenario_id === 'string'
  const nameOk =
    o.scenario_name === undefined || typeof o.scenario_name === 'string'
  const messageOk = o.message === undefined || typeof o.message === 'string'
  return (
    o.event_type === 'scenario_status' &&
    typeof o.status === 'string' &&
    idOk &&
    nameOk &&
    messageOk
  )
}
