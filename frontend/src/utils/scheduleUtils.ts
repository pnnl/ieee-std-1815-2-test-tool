// Schedule utilities — UI-side view model layered on top of the canonical
// `PicsProfile.AI.schedules`. Components read schedules through `viewSchedules`
// and write back via `applySchedulesToProfile`, both of which delegate to the
// canonical helpers. This module owns the UI's date/time/Power-Mode glue.

import type { PicsProfile, AoPoint } from '@/api/generated'
import {
  setSchedulePoints,
  updateSchedule as canonicalUpdateSchedule,
  setScheduleCount,
  viewSchedules,
  flattenAnalogOutputs,
} from '@/profile/canonical'

export interface SchedulePoint {
  timeOffset: number
  actionType: number
  actionIndex: number
  value: number
}

export interface Schedule {
  index: number // display index (matches arrayIndex in canonical AI.schedules)
  identity: number
  priority: number
  startDateTime: Date
  stopDateTime: Date
  repeatInterval: number
  repeatIntervalUnit: number
  numberOfPoints: number
  points: SchedulePoint[]
  weekdayFlags: boolean[]
}

export interface PowerMode {
  name: string
  purpose: string
  aoIndex?: number
}

// Repeat interval unit mapping
export const INTERVAL_UNITS: Record<number, string> = {
  0: 'No Repeat',
  1: 'Seconds',
  2: 'Minutes',
  3: 'Hours',
  4: 'Days',
  5: 'Weeks',
  6: 'Months',
  7: 'Months (Same Day of Week)',
  8: 'Months (Same Day of Week from End)',
}

// Action type mapping
export const ACTION_TYPES: Record<number, string> = {
  0: 'Null (no action)',
  1: 'Set Analog Output',
  2: 'Set Binary Output',
  3: 'Stop Schedule',
}

// Power modes that can be scheduled (grouped by category).
export const POWER_MODES: Record<string, PowerMode[]> = {
  activePower: [
    { name: 'Set Active Power', purpose: 'Set Active Power' },
    { name: 'Coordinated Charge-Discharge', purpose: 'Coord Charge-Dischg' },
    { name: 'Active Power Limit', purpose: 'Active Power Limit' },
    { name: 'Automatic Generation Control', purpose: 'AGC' },
    { name: 'Active Power Smoothing', purpose: 'Active Power Smoothing' },
    { name: 'Volt-Watt', purpose: 'Volt-Watt' },
    { name: 'Frequency-Watt Curve', purpose: 'Freq-Watt Curve' },
    { name: 'Pricing Signal', purpose: 'Price' },
  ],
  reactivePower: [
    { name: 'Constant Vars', purpose: 'Const Vars' },
    { name: 'Constant Power Factor', purpose: 'Constant PF' },
    { name: 'Volt-VAR Control', purpose: 'Volt-Var' },
    { name: 'Watt-VAR', purpose: 'Watt-Var' },
    { name: 'Power Factor Correction', purpose: 'PF Correct' },
  ],
  emergencyModes: [
    { name: 'Voltage Ride-Through', purpose: 'Volt Ride-Through' },
    { name: 'Frequency Ride-Through', purpose: 'Freq Ride-Through' },
    { name: 'Dynamic Reactive Current', purpose: 'Dyn React Curr Supp' },
    { name: 'Dynamic Volt-Watt', purpose: 'Dyn Volt-Watt' },
    { name: 'Frequency-Watt', purpose: 'Freq-Watt' },
  ],
}

// Schedule colors for visualization (by index)
const SCHEDULE_COLORS: string[] = [
  '#3498db',
  '#2ecc71',
  '#e74c3c',
  '#f39c12',
  '#9b59b6',
  '#1abc9c',
  '#e67e22',
  '#34495e',
  '#16a085',
  '#c0392b',
]

// Get color for a schedule index.
export function getScheduleColor(scheduleIndex: number): string {
  return SCHEDULE_COLORS[scheduleIndex % SCHEDULE_COLORS.length]
}

// Format a date/time for display.
export function formatDateTime(date: Date | null): string {
  if (!date) return 'Not set'
  return date.toLocaleString()
}

// Get all AO points whose `purpose` matches the supplied power-mode purpose.
// Reads the canonical `AO.points` plus equipment-grouped AO points.
export function getPointsForMode(
  profile: PicsProfile,
  purpose: string,
): AoPoint[] {
  return flattenAnalogOutputs(profile)
    .filter((p) => p.purpose === purpose)
    .map((p) => {
      // Strip the FlatAoPoint metadata; consumers just want the canonical point.
      const rest: Record<string, unknown> = { ...p }
      delete rest.kind
      delete rest.locator
      return rest as AoPoint
    })
}

// Pull a UI-flavored Schedule[] out of the canonical profile. Provides default
// dates when the canonical zero-encoded "no time" representation comes back.
export function extractSchedulesFromProfile(profile: PicsProfile): Schedule[] {
  const views = viewSchedules(profile)
  return views.map((v) => ({
    index: v.index,
    identity: v.identity,
    priority: v.priority,
    startDateTime: v.startDate ?? new Date(),
    stopDateTime: v.stopDate ?? new Date(Date.now() + 60 * 60 * 1000),
    repeatInterval: v.repeatInterval,
    repeatIntervalUnit: v.repeatIntervalUnit,
    numberOfPoints: v.numberOfPoints,
    points: v.points,
    weekdayFlags: [false, false, false, false, false, false, false],
  }))
}

// Apply a UI Schedule[] back onto the canonical profile. Each Schedule.index
// corresponds to its position in `AI.schedules` (0-based array index). This
// function:
//   - resizes `AI.schedules` to len(schedules)
//   - rewrites every schedule's metadata + action points
// Returns a fresh profile.
export function applySchedulesToProfile(
  profile: PicsProfile,
  schedules: Schedule[],
): PicsProfile {
  let next = setScheduleCount(profile, schedules.length)
  for (let i = 0; i < schedules.length; i++) {
    const s = schedules[i]
    next = canonicalUpdateSchedule(next, i, {
      identity: s.identity,
      priority: s.priority,
      startDate: s.startDateTime,
      stopDate: s.stopDateTime,
      repeatInterval: s.repeatInterval,
      repeatIntervalUnit: s.repeatIntervalUnit,
    })
    next = setSchedulePoints(next, i, s.points)
  }
  return next
}

// Create a brand-new schedule. Used by the Add button — the actual canonical
// insertion happens via `applySchedulesToProfile` once the UI list is full.
export function createEmptySchedule(index: number): Schedule {
  return {
    index,
    identity: index + 1,
    priority: index + 1,
    repeatInterval: 0,
    repeatIntervalUnit: 0,
    numberOfPoints: 0,
    points: [],
    weekdayFlags: [false, false, false, false, false, false, false],
    startDateTime: new Date(),
    stopDateTime: new Date(Date.now() + 60 * 60 * 1000),
  }
}

export type GanttEvent = {
  startDateTime: Date
  stopDateTime: Date
  id: number
  label: string
  priority: number
  groupId: number
}

export function scheduleToEvents(schedule: Schedule): GanttEvent[] {
  return [
    {
      startDateTime: schedule.startDateTime,
      stopDateTime: schedule.stopDateTime,
      id: schedule.index,
      label: `Schedule ${schedule.index}`,
      priority: schedule.priority,
      groupId: schedule.index,
    },
  ]
}
