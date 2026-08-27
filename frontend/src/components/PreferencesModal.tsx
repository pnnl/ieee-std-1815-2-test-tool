import { useState } from 'react'
import { AccessibleDialog } from '@/components/ui/dialog'
import { Button } from '@/components/ui/button'
import { Checkbox } from '@/components/ui/checkbox'
import { Input } from '@/components/ui/input'
import { NumberInput } from '@/components/ui/number-input'
import { Label } from '@/components/ui/label'
import { ScrollArea } from '@/components/ui/scroll-area'
import {
  isValidIpOrLocal,
  isValidPort,
  type Preferences,
} from '../utils/preferences'
import { toast } from 'sonner'

type TestStatus = null | 'testing' | 'success' | 'failed'

interface PreferencesModalProps {
  isOpen: boolean
  onClose: () => void
  preferences: Preferences
  onPreferenceChange: (
    key: keyof Preferences,
    value: Preferences[keyof Preferences],
  ) => void
}

function PreferencesModal({
  isOpen,
  onClose,
  preferences,
  onPreferenceChange,
}: PreferencesModalProps) {
  const [controlStationIpInput, setControlStationIpInput] = useState(
    preferences.controlStationIp || 'local',
  )
  const [controlStationPortInput, setControlStationPortInput] = useState<
    number | string
  >(preferences.controlStationPort || 20000)
  const [outstationIpInput, setOutstationIpInput] = useState(
    preferences.outstationIp || 'local',
  )
  const [outstationPortInput, setOutstationPortInput] = useState<
    number | string
  >(preferences.outstationPort || 20001)

  const [controlStationIpError, setControlStationIpError] = useState('')
  const [controlStationPortError, setControlStationPortError] = useState('')
  const [outstationIpError, setOutstationIpError] = useState('')
  const [outstationPortError, setOutstationPortError] = useState('')

  const [controlStationTestStatus, setControlStationTestStatus] =
    useState<TestStatus>(null)
  const [outstationTestStatus, setOutstationTestStatus] =
    useState<TestStatus>(null)

  const handleIpChange = (
    field: keyof Preferences,
    value: string,
    setInput: (v: string) => void,
    setError: (v: string) => void,
  ) => {
    setInput(value)
    if (isValidIpOrLocal(value)) {
      setError('')
      onPreferenceChange(field, value)
    } else {
      setError('Enter a valid IP address or "local"')
    }
  }

  const handlePortChange = (
    field: keyof Preferences,
    value: number | string,
    setInput: (v: number | string) => void,
    setError: (v: string) => void,
  ) => {
    setInput(value)
    if (isValidPort(value)) {
      setError('')
      onPreferenceChange(
        field,
        typeof value === 'string' ? parseInt(value, 10) : value,
      )
    } else {
      setError('Enter a valid port (1-65535)')
    }
  }

  const testConnection = async (type: 'controlStation' | 'outstation') => {
    const setStatus =
      type === 'controlStation'
        ? setControlStationTestStatus
        : setOutstationTestStatus
    const ip =
      type === 'controlStation' ? controlStationIpInput : outstationIpInput
    const port =
      type === 'controlStation' ? controlStationPortInput : outstationPortInput

    if (!isValidIpOrLocal(ip) || !isValidPort(port)) {
      toast.error(
        'Please enter a valid IP address and port before testing the connection.',
      )
      setStatus('failed')
      setTimeout(() => setStatus(null), 2000)
      return
    }

    setStatus('testing')

    try {
      const host = ip === 'local' ? '127.0.0.1' : ip
      const response = await fetch(
        `/api/health/test-connection?host=${encodeURIComponent(host)}&port=${encodeURIComponent(port)}`,
      )
      if (response.ok) {
        setStatus('success')
      } else {
        toast.error('Error: ' + JSON.stringify(response.json()))
        setStatus('failed')
      }
    } catch {
      toast.error('Failed to test connection.')
      setStatus('failed')
    }

    setTimeout(() => setStatus(null), 2000)
  }

  const getTestButtonLabel = (status: TestStatus) => {
    switch (status) {
      case 'testing':
        return 'Testing...'
      case 'success':
        return 'Connected'
      case 'failed':
        return 'Failed'
      default:
        return 'Test'
    }
  }

  const getTestButtonClass = (status: TestStatus) => {
    switch (status) {
      case 'testing':
        return 'bg-warning text-white hover:bg-warning/90'
      case 'success':
        return 'bg-success text-white hover:bg-success/90'
      case 'failed':
        return 'bg-danger text-white hover:bg-danger/90'
      default:
        return ''
    }
  }

  return (
    <AccessibleDialog
      isOpen={isOpen}
      onClose={onClose}
      title="Preferences"
      description="Editor preferences and DNP3 endpoint configuration for the test runner."
      contentClassName="sm:max-w-2xl max-h-[90vh] flex flex-col"
      footer={
        <Button variant="outline" onClick={onClose}>
          Close
        </Button>
      }
    >
      <ScrollArea className="flex-1 pr-3">
        <div className="flex flex-col gap-6">
          <div className="flex items-start gap-2">
            <Checkbox
              id="auto-expand"
              checked={preferences.autoExpandSections}
              onCheckedChange={() =>
                onPreferenceChange(
                  'autoExpandSections',
                  !preferences.autoExpandSections,
                )
              }
            />
            <div className="flex flex-col gap-0.5">
              <Label
                htmlFor="auto-expand"
                className="text-sm font-medium cursor-pointer"
              >
                Auto-expand sections on tab change
              </Label>
              <p className="text-xs text-muted-foreground">
                Automatically expand all offset sections when switching between
                point tabs
              </p>
            </div>
          </div>

          {/* Control Station */}
          <div className="flex flex-col gap-1.5">
            <p className="font-semibold text-sm">Control Station</p>
            <p className="text-xs text-muted-foreground">
              IP address and port for the control station (DNP3 master)
            </p>
            <div className="flex items-start gap-1.5">
              <div className="flex-1 flex flex-col gap-1">
                <Input
                  value={controlStationIpInput}
                  onChange={(e) =>
                    handleIpChange(
                      'controlStationIp',
                      e.target.value,
                      setControlStationIpInput,
                      setControlStationIpError,
                    )
                  }
                  placeholder="IP or local"
                  className={controlStationIpError ? 'border-danger' : ''}
                />
                {controlStationIpError && (
                  <p className="text-xs text-danger">{controlStationIpError}</p>
                )}
              </div>
              <span className="pt-2 text-sm">:</span>
              <div className="flex flex-col gap-1">
                <NumberInput
                  className="w-[100px]"
                  value={controlStationPortInput}
                  onChange={(val) =>
                    handlePortChange(
                      'controlStationPort',
                      val,
                      setControlStationPortInput,
                      setControlStationPortError,
                    )
                  }
                  min={1}
                  max={65535}
                  hideControls
                  error={controlStationPortError || undefined}
                />
              </div>
              <Button
                variant="outline"
                size="sm"
                className={getTestButtonClass(controlStationTestStatus)}
                onClick={() => testConnection('controlStation')}
                disabled={controlStationTestStatus === 'testing'}
              >
                {getTestButtonLabel(controlStationTestStatus)}
              </Button>
            </div>
          </div>

          {/* Outstation */}
          <div className="flex flex-col gap-1.5">
            <p className="font-semibold text-sm">Outstation</p>
            <p className="text-xs text-muted-foreground">
              IP address and port for the outstation (DNP3 server)
            </p>
            <div className="flex items-start gap-1.5">
              <div className="flex-1 flex flex-col gap-1">
                <Input
                  value={outstationIpInput}
                  onChange={(e) =>
                    handleIpChange(
                      'outstationIp',
                      e.target.value,
                      setOutstationIpInput,
                      setOutstationIpError,
                    )
                  }
                  placeholder="IP or local"
                  className={outstationIpError ? 'border-danger' : ''}
                />
                {outstationIpError && (
                  <p className="text-xs text-danger">{outstationIpError}</p>
                )}
              </div>
              <span className="pt-2 text-sm">:</span>
              <div className="flex flex-col gap-1">
                <NumberInput
                  className="w-[100px]"
                  value={outstationPortInput}
                  onChange={(val) =>
                    handlePortChange(
                      'outstationPort',
                      val,
                      setOutstationPortInput,
                      setOutstationPortError,
                    )
                  }
                  min={1}
                  max={65535}
                  hideControls
                  error={outstationPortError || undefined}
                />
              </div>
              <Button
                variant="outline"
                size="sm"
                className={getTestButtonClass(outstationTestStatus)}
                onClick={() => testConnection('outstation')}
                disabled={outstationTestStatus === 'testing'}
              >
                {getTestButtonLabel(outstationTestStatus)}
              </Button>
            </div>
          </div>
        </div>
      </ScrollArea>
    </AccessibleDialog>
  )
}

export default PreferencesModal
