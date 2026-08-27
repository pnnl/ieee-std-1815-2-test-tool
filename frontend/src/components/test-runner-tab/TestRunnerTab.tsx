import { useState, useRef, useEffect } from 'react'
import {
  ConformanceTestResult,
  LogSource,
  TestRunnerEvent,
  ProcessName,
  ProcessStatus,
  ProcessStatusMap,
  LogEntry,
} from './types'
import { dispatchTestRunnerEvent } from './eventDispatch'
import { formatLogsForDownload, getSourceLabel } from './downloadLogs'
import { triggerBundleDownload } from './exportBundle'
import { Clipboard, Download, ListVideo, Square, X } from 'lucide-react'
import styles from './TestRunnerTab.module.css'
import {
  type Preferences,
  type DeviceUnderTest,
  savePreferences,
} from '../../utils/preferences'
import { ToggleGroup, ToggleGroupItem } from '@/components/ui/toggle-group'

const PROCESS_STATUS_CLASSES: Partial<Record<ProcessStatus, string>> = {
  stopped: styles.statusStopped,
  unknown: styles.statusUnknown,
  starting: styles.statusStarting,
  compiling: styles.statusCompiling,
  started: styles.statusStarted,
  running: styles.statusRunning,
  finished: styles.statusFinished,
  error: styles.statusError,
}
import type { PicsProfile } from '@/api/generated'
import { toast } from 'sonner'
import TestResultsTree from './TestResultsTree'
import { type Scenario, fetchScenarios } from './scenarios'
import { Button } from '../ui/button'

interface TestRunnerTabProps {
  profileData: PicsProfile
  preferences: Preferences
  currentProfileName?: string | null
}

function TestRunnerTab({
  profileData,
  preferences,
  currentProfileName = null,
}: TestRunnerTabProps) {
  const [processStatuses, setProcessStatuses] = useState<ProcessStatusMap>(
    new Map([
      ['test_runner', 'not started'],
      ['control_station', 'not started'],
      ['outstation', 'not started'],
    ]),
  )
  const [jobId, setJobId] = useState<string | null>(null)
  const [logs, setLogs] = useState<LogEntry[]>([])
  const [selectedSources, setSelectedSources] = useState<Set<LogSource>>(
    new Set(),
  )
  const [isConnecting, setIsConnecting] = useState(false)
  const eventSourceRef = useRef<EventSource | null>(null)
  const logContainerRef = useRef<HTMLDivElement | null>(null)
  const logIdCounterRef = useRef(0)
  const [testResults, setTestResults] = useState<
    Record<string, ConformanceTestResult>
  >({})
  const [scenarios, setScenarios] = useState<Scenario[]>([])
  const [enabledScenarios, setEnabledScenarios] = useState<
    Record<string, boolean>
  >(() => {
    return { ...preferences.enabledScenarios }
  })
  const [deviceUnderTest, setDeviceUnderTest] = useState<DeviceUnderTest>(
    preferences.deviceUnderTest ?? 'outstation',
  )

  const handleDeviceUnderTestChange = (device: DeviceUnderTest) => {
    setDeviceUnderTest(device)
    savePreferences({ ...preferences, deviceUnderTest: device })
  }

  const LOG_SOURCES: LogSource[] = [
    'test_runner',
    'control_station',
    'outstation',
    'web_client',
  ]

  const toggleSource = (source: LogSource) => {
    const newSelected = new Set(selectedSources)
    if (newSelected.has(source)) {
      newSelected.delete(source)
    } else {
      newSelected.add(source)
    }
    setSelectedSources(newSelected)
  }

  const filteredLogs = logs.filter(
    (log) => selectedSources.size === 0 || selectedSources.has(log.source),
  )

  function setProcessStatus(process: ProcessName, status: ProcessStatus) {
    setProcessStatuses((prev: ProcessStatusMap) => {
      const next = new Map(prev)
      next.set(process, status)
      return next
    })
  }

  // Auto-scroll to bottom when new logs arrive
  useEffect(() => {
    if (logContainerRef.current) {
      logContainerRef.current.scrollTop = logContainerRef.current.scrollHeight
    }
  }, [logs])

  // Fetch scenarios from API on mount
  useEffect(() => {
    fetchScenarios()
      .then((fetched) => {
        setScenarios(fetched)
        setEnabledScenarios((prev) => {
          const defaults = Object.fromEntries(fetched.map((s) => [s.id, true]))
          return { ...defaults, ...prev }
        })
      })
      .catch((error) => {
        toast.error(
          'Failed to load scenarios: ' +
            (error instanceof Error ? error.message : 'Unknown error'),
        )
      })
  }, [])

  // Cleanup on unmount
  useEffect(() => {
    return () => {
      if (eventSourceRef.current) {
        eventSourceRef.current.close()
      }
    }
  }, [])

  const handleStart = async () => {
    if (!profileData) {
      toast.warning('Please load a profile before starting the test runner')
      return
    }

    if (!Object.values(enabledScenarios).some(Boolean)) {
      toast.warning(
        'Select at least one scenario before starting the test runner',
      )
      return
    }

    setIsConnecting(true)
    setLogs([])
    logIdCounterRef.current = 0
    setTestResults({})
    setProcessStatus('test_runner', 'starting')

    try {
      // Build configs based on preferences
      // "local" means use the reference implementation, otherwise connect to external endpoint
      const controlStationConfig = {
        use_reference_control_station:
          preferences.controlStationIp === '127.0.0.1',
        ip_address: preferences.controlStationIp,
        port: preferences.controlStationPort,
      }

      const outstationConfig = {
        use_reference_outstation: preferences.outstationIp === '127.0.0.1',
        ip_address: preferences.outstationIp,
        port: preferences.outstationPort,
      }

      const response = await fetch('/api/jobs', {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
        },
        body: JSON.stringify({
          control_station_config: controlStationConfig,
          outstation_config: outstationConfig,
          profile: profileData,
          scenario_ids: Object.entries(enabledScenarios)
            .filter(([, enabled]) => enabled)
            .map(([id]) => id),
          device_under_test: deviceUnderTest,
        }),
      })

      if (!response.ok) {
        toast.error('Failed to start test runner: ' + response.statusText)
        throw new Error('Failed to create job')
      }

      const data = await response.json()
      const newJobId = data.job_id
      setJobId(newJobId)

      // Connect to SSE stream
      const eventSource = new EventSource(`/api/jobs/${newJobId}/events`)
      eventSourceRef.current = eventSource

      eventSource.onopen = () => {
        setProcessStatus('test_runner', 'started')
        setIsConnecting(false)
        addLog('web_client', 'Connected to test runner.')
      }

      eventSource.onmessage = (event) => {
        try {
          const eventData = JSON.parse(event.data)
          handleEvent(eventData)
        } catch (e) {
          console.error('Failed to parse event:', e)
        }
      }

      eventSource.onerror = (event: Event) => {
        console.error('SSE error:', event)
        // Close immediately on error to prevent unwanted reconnection loops
        eventSource.close()
        addLog('web_client', `Connection closed due to error: ${event.type}`)
        setIsConnecting(false)
      }
    } catch (error) {
      const errorMessage =
        error instanceof Error ? error.message : 'Unknown error'
      toast.error('Error starting test runner: ' + errorMessage)
      setIsConnecting(false)
      setProcessStatus('test_runner', 'error')
    }
  }

  const handleEvent = (eventData: TestRunnerEvent) => {
    dispatchTestRunnerEvent(eventData, {
      addLog,
      setProcessStatus,
      recordConformanceResult: (result: ConformanceTestResult) => {
        const resultKey = `${result.scenario_id}:${result.test}`
        setTestResults((prev) => ({
          ...prev,
          [resultKey]: result,
        }))
      },
      warnUnknown: (data) => {
        console.warn('Unknown event type or malformed message:', data)
      },
    })
  }

  const addLog = (source: LogSource, message: string) => {
    const timestamp = new Date().toLocaleTimeString()
    const id = `${Date.now()}-${logIdCounterRef.current++}`
    setLogs((prev) => [...prev, { id, timestamp, source, message }])
  }

  const handleStop = async () => {
    setProcessStatus('test_runner', 'stopping')

    try {
      const response = await fetch(`/api/jobs/${jobId}`, {
        method: 'DELETE',
      })

      if (!response.ok) {
        setProcessStatus('test_runner', 'error')
        throw new Error('Failed to stop job')
      } else {
        setProcessStatus('test_runner', 'stopped')
        setProcessStatus('control_station', 'stopped')
        setProcessStatus('outstation', 'stopped')
      }
    } catch (error) {
      setProcessStatus('test_runner', 'error')
      toast.error(
        'Error stopping job: ' +
          (error instanceof Error ? error.message : 'Unknown error'),
      )
    } finally {
      if (eventSourceRef.current) {
        eventSourceRef.current.close()
        eventSourceRef.current = null
      }
      addLog('web_client', 'Test runner stopped.')
      setJobId(null)
      setIsConnecting(false)
    }
  }

  // Derived runner state: hoisted above the handlers so handleToggleTestRun,
  // handleDownload, and the JSX return block all share one derived value.
  const runnerStatus = processStatuses.get('test_runner') || 'unknown'
  const runnerActive =
    runnerStatus === 'running' ||
    runnerStatus === 'starting' ||
    runnerStatus === 'started'
  // hasResults gates both the handleDownload guard and the Download button disabled prop.
  const hasResults = Object.keys(testResults).length > 0

  const handleToggleTestRun = async () => {
    if (runnerActive || isConnecting) {
      await handleStop()
    } else {
      await handleStart()
    }
  }

  const handleCopyLogs = () => {
    // Guard against a race where React state has not flushed the empty state yet.
    if (filteredLogs.length === 0) return
    // Serialize the on-screen filtered view so Copy matches what the user sees.
    const logText = formatLogsForDownload(filteredLogs)
    navigator.clipboard
      .writeText(logText)
      .then(() => {
        toast.success('Logs copied to clipboard')
      })
      .catch((error) => {
        toast.error(
          'Failed to copy logs: ' +
            (error instanceof Error ? error.message : 'Unknown error'),
        )
      })
  }

  const handleClearLogs = () => {
    setLogs([])
    setTestResults({})
    logIdCounterRef.current = 0
  }

  const handleDownload = () => {
    // Guard: button is disabled when hasResults is false, but guard here too
    // against any race where React state has not flushed yet.
    if (!hasResults) return
    // Only include scenarios that were enabled for the run, matching the
    // predicate handleStart sends as scenario_ids. Disabled scenarios have no
    // results, so including them would inflate summary.pending and add phantom
    // scenario entries to results.json and report.md.
    const enabledForRun = scenarios.filter(
      (s) => enabledScenarios[s.id] !== false,
    )
    try {
      triggerBundleDownload({
        scenarios: enabledForRun,
        results: testResults,
        logs,
        profileData,
        profileName: currentProfileName,
        jobId,
        deviceUnderTest,
        hasStarted: runnerStatus !== 'not started',
      })
    } catch (error) {
      toast.error(
        'Failed to download bundle: ' +
          (error instanceof Error ? error.message : 'Unknown error'),
      )
    }
  }

  const getSourceClass = (source: LogSource) => {
    switch (source) {
      case 'control_station':
        return styles.logSourceControlStation
      case 'outstation':
        return styles.logSourceOutstation
      case 'test_runner':
        return styles.logSourceTestRunner
      case 'web_client':
        return styles.logSourceWebClient
    }
  }

  const renderedStatuses = []

  for (const [processName, processStatus] of processStatuses.entries()) {
    renderedStatuses.push(
      <div key={processName} className={styles.statusRow}>
        <div className={styles.statusMeta}>
          <span
            className={`${styles.statusIndicator} ${PROCESS_STATUS_CLASSES[processStatus] ?? ''}`}
          ></span>
          <span className={styles.statusLabel}>
            {processName
              .replace('_', ' ')
              .replace(/\b\w/g, (c) => c.toUpperCase())}
          </span>
        </div>
        <span className={styles.statusText}>{processStatus}</span>
      </div>,
    )
  }

  const startIcon = runnerActive ? (
    <Square size={16} aria-hidden="true" />
  ) : (
    <ListVideo size={24} aria-hidden="true" />
  )

  const getLogMessageColor = (message: string) => {
    if (message.includes('✓')) {
      return '#83d45a' // Green for success
    } else if (
      message.includes('✗') ||
      message.toLowerCase().includes('error') ||
      message.toLowerCase().includes('fail') ||
      message.toLowerCase().includes('panic')
    ) {
      return '#fc7173' // Red for errors
    } else if (message.toLowerCase().includes('warn')) {
      return '#ffc852' // Orange for warnings
    } else if (message.toLowerCase().includes('success')) {
      return '#81ff43' // Green for success
    }

    return '#f2f6f7'
  }

  return (
    <div className={styles.testRunnerTab}>
      <div className={styles.testRunnerBody}>
        <aside className={styles.testRunnerSidebar}>
          <div className={styles.jobInfo}>
            <div className={styles.statusList} aria-live="polite">
              {renderedStatuses}
            </div>
          </div>

          <div className={styles.dutSection}>
            <span className={styles.dutLabel}>Device Under Test</span>
            <ToggleGroup
              value={deviceUnderTest}
              onValueChange={(value) =>
                handleDeviceUnderTestChange(value as DeviceUnderTest)
              }
              aria-label="Device under test"
            >
              <ToggleGroupItem value="control_station">
                Control Station
              </ToggleGroupItem>
              <ToggleGroupItem value="outstation">Outstation</ToggleGroupItem>
            </ToggleGroup>
          </div>

          <div className={styles.testRunnerControls}>
            <button
              className={`${styles.btn} ${styles.startStopButton} ${runnerActive ? styles.btnDanger : styles.btnSuccess}`}
              onClick={handleToggleTestRun}
              disabled={isConnecting}
              aria-label={runnerActive ? 'Stop test run' : 'Start test run'}
              aria-pressed={runnerActive}
            >
              {startIcon}&nbsp;&nbsp;&nbsp;
              {runnerActive ? 'Stop Test Run' : 'Start Test Run'}
            </button>
          </div>

          {scenarios.length > 0 && (
            <TestResultsTree
              scenarios={scenarios}
              results={testResults}
              hasStarted={runnerActive}
              enabledScenarios={enabledScenarios}
              onToggleScenario={(id) =>
                setEnabledScenarios((prev) => {
                  const updated = { ...prev, [id]: !prev[id] }
                  savePreferences({ ...preferences, enabledScenarios: updated })
                  return updated
                })
              }
              onSetAllScenarios={(enabled) =>
                setEnabledScenarios(() => {
                  const updated = Object.fromEntries(
                    scenarios.map((scenario) => [scenario.id, enabled]),
                  )
                  savePreferences({ ...preferences, enabledScenarios: updated })
                  return updated
                })
              }
            />
          )}
        </aside>

        <div className={styles.testRunnerLogs}>
          <div className={styles.logControls}>
            <div className={styles.logFilterGroup}>
              <span className={styles.filterLabel}>Filter:</span>
              {LOG_SOURCES.map((source) => (
                <button
                  key={source}
                  className={`${styles.logFilterBtn} ${getSourceClass(source)} ${selectedSources.has(source) ? styles.active : ''}`}
                  onClick={() => toggleSource(source)}
                >
                  {getSourceLabel(source)}
                </button>
              ))}
            </div>
            <span style={{ display: 'flex', gap: '8px' }}>
              <Button
                variant="outline"
                size="xs"
                onClick={handleCopyLogs}
                disabled={filteredLogs.length === 0}
              >
                <Clipboard aria-hidden="true" /> Copy Logs
              </Button>
              <Button
                variant="outline"
                size="xs"
                onClick={handleDownload}
                disabled={!hasResults}
              >
                <Download aria-hidden="true" /> Download
              </Button>
              <Button
                variant="outline"
                size="xs"
                onClick={handleClearLogs}
                disabled={logs.length === 0}
              >
                <X aria-hidden="true" /> Clear Logs
              </Button>
            </span>
          </div>
          <div className={styles.logContainer} ref={logContainerRef}>
            {logs.length === 0 ? (
              <div className={styles.logEmpty}>
                <p>
                  No logs yet. Click &quot;Start&quot; to begin the test runner.
                </p>
              </div>
            ) : filteredLogs.length === 0 ? (
              <div className={styles.logEmpty}>
                <p>No logs match the selected filters.</p>
              </div>
            ) : (
              filteredLogs.map((log) => (
                <div
                  key={log.id}
                  className={`${styles.logEntry} ${getSourceClass(log.source)}`}
                >
                  <span className={styles.logTimestamp}>{log.timestamp}</span>
                  <span
                    className={`${styles.logSource} ${getSourceClass(log.source)}`}
                  >
                    [{getSourceLabel(log.source)}]
                  </span>
                  <span
                    style={{ color: getLogMessageColor(log.message) }}
                    className={styles.logMessage}
                  >
                    {log.message}
                  </span>
                </div>
              ))
            )}
          </div>
        </div>
      </div>
    </div>
  )
}

export default TestRunnerTab
