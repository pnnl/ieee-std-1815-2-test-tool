import {
  type ConformanceTestResult,
  type LogSource,
  type ProcessName,
  type ProcessStatus,
  type TestRunnerEvent,
  type TestRunnerScenarioStatusMessage,
  isConformanceTestResult,
  isScenarioStatusMessage,
  isStatusMessage,
} from './types'

export interface EventDispatchCallbacks {
  addLog: (source: LogSource, message: string) => void
  setProcessStatus: (process: ProcessName, status: ProcessStatus) => void
  recordConformanceResult: (result: ConformanceTestResult) => void
  warnUnknown: (eventData: TestRunnerEvent) => void
}

/**
 * Format a scenario_status message as a single log line. Both `scenario_id`
 * and `scenario_name` may be absent (e.g. for the terminal `all_complete`
 * envelope), so the formatter keys on `status` and prefixes with the
 * scenario name only when present.
 *
 * Examples:
 *   { status: 'error', scenario_name: 'Outstation ingests PICS', message: 'Broken pipe' }
 *     -> '[Outstation ingests PICS] error: Broken pipe'
 *   { status: 'all_complete', message: 'All scenarios complete' }
 *     -> 'all_complete: All scenarios complete'
 *   { status: 'passed', scenario_name: 'Outstation ingests PICS' }
 *     -> '[Outstation ingests PICS] passed'
 */
function formatScenarioStatusLog(
  message: TestRunnerScenarioStatusMessage,
): string {
  const prefix = message.scenario_name ? `[${message.scenario_name}] ` : ''
  const body = message.message
    ? `${message.status}: ${message.message}`
    : message.status
  return `${prefix}${body}`
}

/**
 * Pure dispatcher for SSE events arriving on the test runner stream. The
 * component supplies callbacks that map effects onto React state setters.
 * Extracted from `TestRunnerTab.handleEvent` so the branch logic and type
 * guards are testable without rendering the component.
 */
export function dispatchTestRunnerEvent(
  eventData: TestRunnerEvent,
  callbacks: EventDispatchCallbacks,
): void {
  const { source, event_type, message } = eventData

  if (
    event_type === 'log' &&
    message &&
    'message' in message &&
    typeof message.message === 'string'
  ) {
    callbacks.addLog(source, message.message)
    return
  }

  if (event_type === 'status_update' && isStatusMessage(message)) {
    callbacks.setProcessStatus(message.process, message.status)
    return
  }

  if (
    event_type === 'conformance_test_result' &&
    isConformanceTestResult(message)
  ) {
    callbacks.recordConformanceResult(message)
    return
  }

  if (event_type === 'scenario_status' && isScenarioStatusMessage(message)) {
    callbacks.addLog('test_runner', formatScenarioStatusLog(message))
    return
  }

  callbacks.warnUnknown(eventData)
}
