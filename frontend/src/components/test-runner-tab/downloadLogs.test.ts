import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import {
  formatLogsForDownload,
  formatLine,
  getSourceLabel,
  triggerDownloadWithBlob,
} from './downloadLogs'
import type { LogEntry, LogSource } from './types'

function makeLog(overrides: Partial<LogEntry> & { id: string }): LogEntry {
  return {
    timestamp: '12:00:00 PM',
    source: 'test_runner',
    message: 'default message',
    ...overrides,
  }
}

// ---------------------------------------------------------------------------
// getSourceLabel
// ---------------------------------------------------------------------------

describe('getSourceLabel', () => {
  it('returns the correct label for every known LogSource', () => {
    expect(getSourceLabel('control_station')).toBe('Control station')
    expect(getSourceLabel('outstation')).toBe('Outstation')
    expect(getSourceLabel('test_runner')).toBe('Test runner')
    expect(getSourceLabel('web_client')).toBe('Web client')
  })

  it('returns the raw source string for an unknown source (no [undefined])', () => {
    // Cast through unknown to simulate a future/unknown LogSource at runtime.
    const unknown = 'future_source' as unknown as LogSource
    expect(getSourceLabel(unknown)).toBe('future_source')
  })
})

// ---------------------------------------------------------------------------
// formatLine
// ---------------------------------------------------------------------------

describe('formatLine', () => {
  it('formats a single entry as "<timestamp> [<label>] <message>"', () => {
    const log = makeLog({
      id: '1',
      timestamp: '12:00:01 PM',
      source: 'test_runner',
      message: 'started',
    })
    expect(formatLine(log)).toBe('12:00:01 PM [Test runner] started')
  })

  it('uses getSourceLabel for every known source variant', () => {
    const sources: LogSource[] = [
      'control_station',
      'outstation',
      'test_runner',
      'web_client',
    ]
    const expected = [
      'Control station',
      'Outstation',
      'Test runner',
      'Web client',
    ]
    sources.forEach((source, i) => {
      const log = makeLog({
        id: String(i),
        timestamp: 'T',
        source,
        message: 'msg',
      })
      expect(formatLine(log)).toBe(`T [${expected[i]}] msg`)
    })
  })

  it('collapses an embedded LF to a single space (line-count invariant)', () => {
    const log = makeLog({ id: '1', timestamp: 'T', message: 'line1\nline2' })
    expect(formatLine(log)).toBe('T [Test runner] line1 line2')
    // Must be exactly one line
    expect(formatLine(log).split('\n')).toHaveLength(1)
  })

  it('collapses an embedded CRLF to a single space', () => {
    const log = makeLog({ id: '1', timestamp: 'T', message: 'line1\r\nline2' })
    expect(formatLine(log)).toBe('T [Test runner] line1 line2')
  })

  it('collapses multiple consecutive newlines to a single space', () => {
    const log = makeLog({ id: '1', timestamp: 'T', message: 'a\n\nb' })
    expect(formatLine(log)).toBe('T [Test runner] a b')
  })

  it('preserves square brackets in the message without confusing the label field', () => {
    const log = makeLog({
      id: '1',
      timestamp: 'T',
      message: '[error] something [bad]',
    })
    expect(formatLine(log)).toBe('T [Test runner] [error] something [bad]')
  })
})

// ---------------------------------------------------------------------------
// formatLogsForDownload
// ---------------------------------------------------------------------------

describe('formatLogsForDownload', () => {
  it('returns an empty string when the logs array is empty', () => {
    expect(formatLogsForDownload([])).toBe('')
  })

  it('formats a single entry correctly', () => {
    const logs: LogEntry[] = [
      makeLog({
        id: '1',
        timestamp: '12:00:01 PM',
        source: 'test_runner',
        message: 'started',
      }),
    ]
    expect(formatLogsForDownload(logs)).toBe(
      '12:00:01 PM [Test runner] started',
    )
  })

  it('joins multiple entries with a single newline and no trailing newline', () => {
    const logs: LogEntry[] = [
      makeLog({
        id: '1',
        timestamp: '12:00:01 PM',
        source: 'test_runner',
        message: 'started',
      }),
      makeLog({
        id: '2',
        timestamp: '12:00:02 PM',
        source: 'control_station',
        message: 'hello cs',
      }),
      makeLog({
        id: '3',
        timestamp: '12:00:03 PM',
        source: 'outstation',
        message: 'hello os',
      }),
    ]
    const out = formatLogsForDownload(logs)
    expect(out).toBe(
      '12:00:01 PM [Test runner] started\n' +
        '12:00:02 PM [Control station] hello cs\n' +
        '12:00:03 PM [Outstation] hello os',
    )
    expect(out.endsWith('\n')).toBe(false)
  })

  it('resolves each LogSource variant through the exported getSourceLabel', () => {
    const sources: LogSource[] = [
      'control_station',
      'outstation',
      'test_runner',
      'web_client',
    ]
    const logs: LogEntry[] = sources.map((source, idx) =>
      makeLog({
        id: String(idx + 1),
        timestamp: `12:00:0${idx + 1} PM`,
        source,
        message: `m-${source}`,
      }),
    )
    const lines = formatLogsForDownload(logs).split('\n')
    expect(lines).toEqual([
      '12:00:01 PM [Control station] m-control_station',
      '12:00:02 PM [Outstation] m-outstation',
      '12:00:03 PM [Test runner] m-test_runner',
      '12:00:04 PM [Web client] m-web_client',
    ])
  })

  it('N entries produce exactly N lines (newline-in-message invariant)', () => {
    const logs: LogEntry[] = [
      makeLog({ id: '1', timestamp: 'T', message: 'multi\nline\nmessage' }),
      makeLog({ id: '2', timestamp: 'T', message: 'normal message' }),
    ]
    const out = formatLogsForDownload(logs)
    // Two entries must produce exactly two lines.
    expect(out.split('\n')).toHaveLength(2)
  })

  it('1000 entries yield exactly 1000 lines', () => {
    const logs: LogEntry[] = Array.from({ length: 1000 }, (_, i) =>
      makeLog({
        id: String(i),
        timestamp: 'T',
        source: 'test_runner',
        message: `msg-${i}`,
      }),
    )
    const lines = formatLogsForDownload(logs).split('\n')
    expect(lines).toHaveLength(1000)
    // Spot-check first and last lines for exact format
    expect(lines[0]).toBe('T [Test runner] msg-0')
    expect(lines[999]).toBe('T [Test runner] msg-999')
  })

  it('unknown source does not produce [undefined] in output', () => {
    const logs: LogEntry[] = [
      makeLog({
        id: '1',
        source: 'future_source' as unknown as LogSource,
        message: 'hi',
      }),
    ]
    const out = formatLogsForDownload(logs)
    expect(out).not.toContain('undefined')
    expect(out).toBe('12:00:00 PM [future_source] hi')
  })
})

// ---------------------------------------------------------------------------
// triggerDownloadWithBlob: shared download primitive
// ---------------------------------------------------------------------------

describe('triggerDownloadWithBlob', () => {
  let createObjectURL: ReturnType<typeof vi.fn>
  let revokeObjectURL: ReturnType<typeof vi.fn>
  let appendChildSpy: ReturnType<typeof vi.spyOn>
  let removeChildSpy: ReturnType<typeof vi.spyOn>
  let clickSpy: ReturnType<typeof vi.spyOn>

  beforeEach(() => {
    createObjectURL = vi.fn().mockReturnValue('blob:shared-util-url')
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

  it('calls createObjectURL, appends anchor, clicks, removes anchor', () => {
    const blob = new Blob(['data'], { type: 'text/plain' })
    triggerDownloadWithBlob(blob, 'file.txt')
    expect(createObjectURL).toHaveBeenCalledOnce()
    expect(appendChildSpy).toHaveBeenCalledOnce()
    expect(clickSpy).toHaveBeenCalledOnce()
    expect(removeChildSpy).toHaveBeenCalledOnce()
  })

  it('schedules revokeObjectURL asynchronously (deferred via setTimeout)', () => {
    vi.useFakeTimers()
    const blob = new Blob(['data'], { type: 'text/plain' })
    triggerDownloadWithBlob(blob, 'file.txt')
    expect(revokeObjectURL).not.toHaveBeenCalled()
    vi.runAllTimers()
    expect(revokeObjectURL).toHaveBeenCalledWith('blob:shared-util-url')
    vi.useRealTimers()
  })

  it('removes anchor even when click() throws (try/finally teardown)', () => {
    clickSpy.mockImplementation(() => {
      throw new Error('click boom')
    })
    const blob = new Blob(['data'], { type: 'text/plain' })
    expect(() => triggerDownloadWithBlob(blob, 'file.txt')).toThrow(
      'click boom',
    )
    expect(removeChildSpy).toHaveBeenCalledOnce()
  })
})
