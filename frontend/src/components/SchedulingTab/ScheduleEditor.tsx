import { useState, useEffect } from 'react'
import { Checkbox } from '@/components/ui/checkbox'
import { Label } from '@/components/ui/label'
import { NumberInput } from '@/components/ui/number-input'
import { NativeSelect } from '@/components/ui/native-select'
import { Input } from '@/components/ui/input'
import { Separator } from '@/components/ui/separator'
import { ScrollArea } from '@/components/ui/scroll-area'
import SchedulePointEditor from './SchedulePointEditor'
import {
  INTERVAL_UNITS,
  POWER_MODES,
  getScheduleColor,
  getPointsForMode,
  Schedule,
  PowerMode,
  SchedulePoint,
} from '../../utils/scheduleUtils'
import type { AoPoint, PicsProfile } from '@/api/generated'

interface ScheduleEditorProps {
  schedule: Schedule | null
  onChange: (updatedSchedule: Schedule) => void
  profileData: PicsProfile
}

function ScheduleEditor({
  schedule,
  onChange,
  profileData,
}: ScheduleEditorProps) {
  const [formData, setFormData] = useState<Schedule | null>(schedule)

  useEffect(() => {
    if (schedule) {
      setFormData(schedule)
    }
  }, [schedule])

  if (!schedule || !formData || formData.index === undefined) {
    return (
      <div className="flex flex-col items-center justify-center h-[300px] text-muted-foreground">
        <h3 className="text-lg font-semibold">No Schedule Selected</h3>
        <p className="text-sm">
          Select a schedule from the list or create a new one.
        </p>
      </div>
    )
  }

  const color = getScheduleColor(schedule.index)

  const handleChange = <K extends keyof Schedule>(
    field: K,
    value: Schedule[K],
  ) => {
    const updated: Schedule = { ...formData, [field]: value }
    setFormData(updated)
    onChange(updated)
  }

  const handleDateTimeChange = (
    type: 'start' | 'stop',
    dateTimeString: string,
  ) => {
    if (!dateTimeString) return

    const date = new Date(dateTimeString)

    const updated = { ...formData }
    if (type === 'start') {
      updated.startDateTime = date
    } else {
      updated.stopDateTime = date
    }

    setFormData(updated)
    onChange(updated)
  }

  const handlePointsChange = (points: SchedulePoint[]) => {
    const updated = {
      ...formData,
      points,
      numberOfPoints: points.length,
    }
    setFormData(updated)
    onChange(updated)
  }

  const getModeAoIndices = (purpose: string): number[] => {
    const modePoints = getPointsForMode(profileData, purpose)
    return modePoints.map((p: AoPoint) => p.point_index)
  }

  const isModeSelected = (purpose: string): boolean => {
    const modeAoIndices = getModeAoIndices(purpose)
    return (formData.points || []).some((p) =>
      modeAoIndices.includes(p.actionIndex),
    )
  }

  const handleModeToggle = (mode: PowerMode, checked: boolean) => {
    const currentPoints = formData.points || []
    const modeAoIndices = getModeAoIndices(mode.purpose)

    if (checked) {
      const newPoints = modeAoIndices.map((aoIndex) => ({
        timeOffset: 0,
        actionType: 1,
        actionIndex: aoIndex,
        value: 0,
      }))
      const existingIndices = new Set(currentPoints.map((p) => p.actionIndex))
      const uniqueNewPoints = newPoints.filter(
        (p) => !existingIndices.has(p.actionIndex),
      )
      handlePointsChange([...currentPoints, ...uniqueNewPoints])
    } else {
      const filteredPoints = currentPoints.filter(
        (p) => !modeAoIndices.includes(p.actionIndex),
      )
      handlePointsChange(filteredPoints)
    }
  }

  const formatDateForInput = (date: Date | null): string => {
    if (!date) return ''
    const d = new Date(date)
    const year = d.getFullYear()
    const month = String(d.getMonth() + 1).padStart(2, '0')
    const day = String(d.getDate()).padStart(2, '0')
    const hours = String(d.getHours()).padStart(2, '0')
    const minutes = String(d.getMinutes()).padStart(2, '0')
    return `${year}-${month}-${day}T${hours}:${minutes}`
  }

  const stopTimeInvalid = formData.stopDateTime < formData.startDateTime

  const intervalUnitOptions = Object.entries(INTERVAL_UNITS).map(
    ([value, label]) => ({
      value,
      label,
    }),
  )

  return (
    <div className="flex flex-col h-full">
      <div
        className="flex items-center p-2 border-b border-border bg-muted"
        style={{ borderLeft: `3px solid ${color}` }}
      >
        <h4 className="text-base font-semibold">Schedule {schedule.index}</h4>
      </div>

      <ScrollArea className="flex-1 min-h-0">
        <div className="flex flex-col gap-6 p-4">
          {/* Basic Info */}
          <div>
            <p className="font-semibold text-sm mb-2">Basic Information</p>
            <div className="grid grid-cols-3 gap-3">
              <NumberInput
                label="Schedule Index"
                value={formData.index}
                disabled
                size="xs"
              />
              <NumberInput
                label="Identity"
                value={formData.identity}
                onChange={(val) => handleChange('identity', Number(val) || 0)}
                min={0}
                size="xs"
              />
              <NumberInput
                label="Priority (Lower = higher)"
                value={formData.priority}
                onChange={(val) => handleChange('priority', Number(val) || 0)}
                min={0}
                max={99}
                size="xs"
              />
            </div>
          </div>

          <Separator />

          {/* Timing */}
          <div>
            <p className="font-semibold text-sm mb-2">Schedule Timing</p>
            <div className="grid grid-cols-2 gap-3">
              <div className="flex flex-col gap-1">
                <Label className="text-xs">Start Date/Time</Label>
                <Input
                  type="datetime-local"
                  value={formatDateForInput(formData.startDateTime)}
                  onChange={(e) =>
                    handleDateTimeChange('start', e.currentTarget.value)
                  }
                  className="h-7 text-xs"
                />
              </div>
              <div className="flex flex-col gap-1">
                <Label className="text-xs">Stop Date/Time</Label>
                <Input
                  type="datetime-local"
                  value={formatDateForInput(formData.stopDateTime)}
                  onChange={(e) =>
                    handleDateTimeChange('stop', e.currentTarget.value)
                  }
                  className="h-7 text-xs"
                  aria-invalid={stopTimeInvalid}
                />
                {stopTimeInvalid && (
                  <p className="text-xs text-destructive">
                    Stop time must be after start time
                  </p>
                )}
              </div>
            </div>
          </div>

          <Separator />

          {/* Repeat Settings */}
          <div>
            <p className="font-semibold text-sm mb-2">Repeat Settings</p>
            <div className="grid grid-cols-2 gap-3 mb-3">
              <NumberInput
                label="Repeat Interval"
                value={formData.repeatInterval}
                onChange={(val) =>
                  handleChange('repeatInterval', Number(val) || 0)
                }
                min={0}
                size="xs"
              />
              <NativeSelect
                label="Interval Unit"
                value={String(formData.repeatIntervalUnit)}
                onChange={(e) =>
                  handleChange(
                    'repeatIntervalUnit',
                    parseInt(e.currentTarget.value),
                  )
                }
                data={intervalUnitOptions}
                size="sm"
              />
            </div>

            <p className="text-xs font-medium text-muted-foreground mb-1.5">
              Repeat on:
            </p>
            <div className="flex items-center gap-3">
              {['Sun', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat'].map(
                (day, idx) => (
                  <div key={day} className="flex items-center gap-1">
                    <Checkbox
                      id={`day-${idx}`}
                      checked={formData.weekdayFlags?.[idx] || false}
                      onCheckedChange={(checked) => {
                        const flags = [
                          ...(formData.weekdayFlags || [
                            false,
                            false,
                            false,
                            false,
                            false,
                            false,
                            false,
                          ]),
                        ]
                        flags[idx] = !!checked
                        handleChange('weekdayFlags', flags)
                      }}
                      className="h-3.5 w-3.5"
                    />
                    <Label
                      htmlFor={`day-${idx}`}
                      className="text-xs cursor-pointer"
                    >
                      {day}
                    </Label>
                  </div>
                ),
              )}
            </div>
          </div>

          <Separator />

          {/* Power Modes */}
          <div>
            <p className="font-semibold text-sm mb-1">Power Modes</p>
            <p className="text-xs text-muted-foreground mb-3">
              Select modes that this schedule will control.
            </p>

            <div className="grid grid-cols-1 sm:grid-cols-3 gap-3">
              <div className="border rounded-sm p-3">
                <p className="text-xs font-semibold uppercase text-muted-foreground mb-2">
                  Active Power
                </p>
                <div className="flex flex-col gap-1">
                  {POWER_MODES.activePower.map((mode: PowerMode) => (
                    <div
                      key={mode.purpose}
                      className="flex items-center gap-1.5"
                    >
                      <Checkbox
                        id={`mode-${mode.purpose}`}
                        checked={isModeSelected(mode.purpose)}
                        onCheckedChange={(checked) =>
                          handleModeToggle(mode, !!checked)
                        }
                        className="h-3.5 w-3.5"
                      />
                      <Label
                        htmlFor={`mode-${mode.purpose}`}
                        className="text-xs cursor-pointer"
                      >
                        {mode.name}
                      </Label>
                    </div>
                  ))}
                </div>
              </div>

              <div className="border rounded-sm p-3">
                <p className="text-xs font-semibold uppercase text-muted-foreground mb-2">
                  Reactive Power
                </p>
                <div className="flex flex-col gap-1">
                  {POWER_MODES.reactivePower.map((mode: PowerMode) => (
                    <div
                      key={mode.purpose}
                      className="flex items-center gap-1.5"
                    >
                      <Checkbox
                        id={`mode-${mode.purpose}`}
                        checked={isModeSelected(mode.purpose)}
                        onCheckedChange={(checked) =>
                          handleModeToggle(mode, !!checked)
                        }
                        className="h-3.5 w-3.5"
                      />
                      <Label
                        htmlFor={`mode-${mode.purpose}`}
                        className="text-xs cursor-pointer"
                      >
                        {mode.name}
                      </Label>
                    </div>
                  ))}
                </div>
              </div>

              <div className="border rounded-sm p-3">
                <p className="text-xs font-semibold uppercase text-muted-foreground mb-2">
                  Emergency
                </p>
                <div className="flex flex-col gap-1">
                  {POWER_MODES.emergencyModes.map((mode: PowerMode) => (
                    <div
                      key={mode.purpose}
                      className="flex items-center gap-1.5"
                    >
                      <Checkbox
                        id={`mode-${mode.purpose}`}
                        checked={isModeSelected(mode.purpose)}
                        onCheckedChange={(checked) =>
                          handleModeToggle(mode, !!checked)
                        }
                        className="h-3.5 w-3.5"
                      />
                      <Label
                        htmlFor={`mode-${mode.purpose}`}
                        className="text-xs cursor-pointer"
                      >
                        {mode.name}
                      </Label>
                    </div>
                  ))}
                </div>
              </div>
            </div>
          </div>

          <Separator />

          {/* Schedule Points */}
          <div>
            <p className="font-semibold text-sm mb-1">Action Points</p>
            <p className="text-xs text-muted-foreground mb-3">
              Define actions that execute at specific time offsets from the
              schedule start.
            </p>
            <SchedulePointEditor
              points={formData.points || []}
              onChange={handlePointsChange}
            />
          </div>
        </div>
      </ScrollArea>
    </div>
  )
}

export default ScheduleEditor
