import { describe, it, expect, vi, type Mock } from 'vitest'
import {
  dispatchTestRunnerEvent,
  type EventDispatchCallbacks,
} from './eventDispatch'
import type { TestRunnerEvent } from './types'

type MockedCallbacks = {
  [K in keyof EventDispatchCallbacks]: Mock<EventDispatchCallbacks[K]>
}

function makeCallbacks(): MockedCallbacks {
  return {
    addLog: vi.fn<EventDispatchCallbacks['addLog']>(),
    setProcessStatus: vi.fn<EventDispatchCallbacks['setProcessStatus']>(),
    recordConformanceResult:
      vi.fn<EventDispatchCallbacks['recordConformanceResult']>(),
    warnUnknown: vi.fn<EventDispatchCallbacks['warnUnknown']>(),
  }
}

describe('dispatchTestRunnerEvent', () => {
  it('renders per-scenario status with scenario name prefix', () => {
    const callbacks = makeCallbacks()
    const event: TestRunnerEvent = {
      event_type: 'scenario_status',
      source: 'test_runner',
      message: {
        event_type: 'scenario_status',
        message:
          'Failed to send scenario command: Failed to write to control station stdin: Broken pipe (os error 32)',
        scenario_id: 'configuration',
        scenario_name: 'Configuration',
        status: 'error',
      },
    }

    dispatchTestRunnerEvent(event, callbacks)

    expect(callbacks.warnUnknown).not.toHaveBeenCalled()
    expect(callbacks.addLog).toHaveBeenCalledTimes(1)
    expect(callbacks.addLog).toHaveBeenCalledWith(
      'test_runner',
      '[Configuration] error: Failed to send scenario command: Failed to write to control station stdin: Broken pipe (os error 32)',
    )
  })

  it('renders terminal all_complete envelope without scenario_id or scenario_name', () => {
    const callbacks = makeCallbacks()
    const event: TestRunnerEvent = {
      event_type: 'scenario_status',
      source: 'test_runner',
      message: {
        event_type: 'scenario_status',
        message: 'All scenarios complete',
        status: 'all_complete',
      },
    }

    dispatchTestRunnerEvent(event, callbacks)

    expect(callbacks.warnUnknown).not.toHaveBeenCalled()
    expect(callbacks.addLog).toHaveBeenCalledTimes(1)
    expect(callbacks.addLog).toHaveBeenCalledWith(
      'test_runner',
      'all_complete: All scenarios complete',
    )
  })

  it('accepts arbitrary future status values without code changes', () => {
    const callbacks = makeCallbacks()
    const event: TestRunnerEvent = {
      event_type: 'scenario_status',
      source: 'test_runner',
      message: {
        event_type: 'scenario_status',
        message: 'Scenario started',
        scenario_id: 'configuration',
        scenario_name: 'Configuration',
        status: 'running',
      },
    }

    dispatchTestRunnerEvent(event, callbacks)

    expect(callbacks.warnUnknown).not.toHaveBeenCalled()
    expect(callbacks.addLog).toHaveBeenCalledWith(
      'test_runner',
      '[Configuration] running: Scenario started',
    )
  })

  it('falls back to status alone when scenario_status message text is missing', () => {
    const callbacks = makeCallbacks()
    const event: TestRunnerEvent = {
      event_type: 'scenario_status',
      source: 'test_runner',
      message: {
        event_type: 'scenario_status',
        scenario_id: 'configuration',
        scenario_name: 'Configuration',
        status: 'passed',
      },
    }

    dispatchTestRunnerEvent(event, callbacks)

    expect(callbacks.warnUnknown).not.toHaveBeenCalled()
    expect(callbacks.addLog).toHaveBeenCalledWith(
      'test_runner',
      '[Configuration] passed',
    )
  })

  it('routes log events to addLog with the event source', () => {
    const callbacks = makeCallbacks()
    const event: TestRunnerEvent = {
      event_type: 'log',
      source: 'control_station',
      message: { event_type: 'log', message: 'hello' },
    }

    dispatchTestRunnerEvent(event, callbacks)

    expect(callbacks.addLog).toHaveBeenCalledWith('control_station', 'hello')
    expect(callbacks.warnUnknown).not.toHaveBeenCalled()
  })

  it('routes status_update events to setProcessStatus', () => {
    const callbacks = makeCallbacks()
    const event: TestRunnerEvent = {
      event_type: 'status_update',
      source: 'test_runner',
      message: {
        event_type: 'status_update',
        process: 'control_station',
        status: 'running',
      },
    }

    dispatchTestRunnerEvent(event, callbacks)

    expect(callbacks.setProcessStatus).toHaveBeenCalledWith(
      'control_station',
      'running',
    )
    expect(callbacks.warnUnknown).not.toHaveBeenCalled()
  })

  it('warns on truly unknown event types', () => {
    const callbacks = makeCallbacks()
    const event = {
      event_type: 'mystery_meat',
      source: 'test_runner',
      message: { event_type: 'mystery_meat' },
    } as unknown as TestRunnerEvent

    dispatchTestRunnerEvent(event, callbacks)

    expect(callbacks.warnUnknown).toHaveBeenCalledTimes(1)
    expect(callbacks.addLog).not.toHaveBeenCalled()
  })
})
