import type { LogEntry, LogSource } from './types'

/**
 * Human-readable label for each log source. Exported so the component,
 * the formatter, and tests all share one definition.
 *
 * The `default` branch returns the raw source string so an unknown or
 * future LogSource value never produces `[undefined]` in output.
 */
export function getSourceLabel(source: LogSource): string {
  switch (source) {
    case 'control_station':
      return 'Control station'
    case 'outstation':
      return 'Outstation'
    case 'test_runner':
      return 'Test runner'
    case 'web_client':
      return 'Web client'
    default:
      // Satisfy TypeScript exhaustiveness while being future-safe at runtime.
      return String(source)
  }
}

/**
 * Canonical single-line format for a log entry:
 *
 *   <timestamp> [<source-label>] <message>
 *
 * Newlines inside `message` are collapsed to a single space so that N
 * entries always produce exactly N output lines. The `.log` format is
 * line-oriented; a raw embedded newline would silently split one entry
 * into multiple lines and confuse any consumer that splits on `\n`.
 */
export function formatLine(log: LogEntry): string {
  const sanitized = log.message.replace(/[\r\n]+/g, ' ')
  return `${log.timestamp} [${getSourceLabel(log.source)}] ${sanitized}`
}

/**
 * Serialize an array of log entries to plain text, one line per entry,
 * joined with `\n`. No trailing newline. Operates on whatever array is
 * passed (the caller is responsible for passing the filtered view).
 */
export function formatLogsForDownload(logs: LogEntry[]): string {
  return logs.map(formatLine).join('\n')
}

/**
 * Shared browser-download primitive: createObjectURL + anchor click + deferred
 * revokeObjectURL. Callers are responsible for constructing the Blob with the
 * appropriate MIME type and content. The revoke is deferred via setTimeout so
 * the browser's download queue can consume the URL before it is released
 * (revoking synchronously after click can produce a zero-byte download on
 * Safari and some Chromium builds).
 */
export function triggerDownloadWithBlob(blob: Blob, filename: string): void {
  const url = URL.createObjectURL(blob)
  const anchor = document.createElement('a')
  anchor.href = url
  anchor.download = filename
  document.body.appendChild(anchor)
  try {
    anchor.click()
  } finally {
    document.body.removeChild(anchor)
    setTimeout(() => URL.revokeObjectURL(url), 0)
  }
}
