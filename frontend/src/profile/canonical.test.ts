// Regression tests for canonical schedule helpers.
//
// Issue #327 ("Screen Blanks"): clicking a SchedulingTab control after loading
// the full profile crashed with:
//
//   TypeError: Cannot set properties of undefined (setting 'value')
//     at setSchedulePoints (canonical.ts)
//     at applySchedulesToProfile (scheduleUtils.ts)
//
// Root cause: when a canonical AiSchedule's parallel arrays (time_offsets,
// action_types, action_indexes, values) are empty, the legacy `cloneClamp`
// helper deliberately refused to grow (no template to clone from).
// `setSchedulePoints` then indexed into those still-empty arrays and wrote
// `.value` on undefined.
//
// The first cut of the fix synthesised/cloned AiPoints to grow the arrays
// but produced wrong `point_index` values — the backend treats `point_index`
// as the DNP3 AI index (see `backend/src/common/src/profile/indexed_db.rs:31`),
// so the saved profile would have been semantically invalid.
//
// The canonical index scheme (mirroring the Rust `AiSchedule::iter_points`
// order in `backend/src/common/src/profile/profile.rs`) is:
//
//   slot k of array a => point_index = number_of_points.point_index + 1 + a + 4*k
//
// where arrayOrdinal a is 0=time_offsets, 1=action_types, 2=action_indexes,
// 3=values. For `full.json` schedule 0, number_of_points.point_index = 3011,
// giving bases 3012/3013/3014/3015 and stride 4 — matches the on-disk data.
//
// These tests reproduce the crash AND guard correctness of the synthesised
// `point_index` values.

import { describe, it, expect } from 'vitest'
import type { AiPoint, AiSchedule, PicsProfile } from '@/api/generated'
import { setSchedulePoints } from './canonical'
import {
  applySchedulesToProfile,
  extractSchedulesFromProfile,
} from '../utils/scheduleUtils'
// Vite's JSON import — gives us the real canonical full profile the user
// reported the crash against. The `assert` is omitted because Vite handles
// JSON natively.
import fullProfileJson from '../../../data/profiles/full.json'

// --- Test fixtures ---------------------------------------------------------

function makePoint(overrides: Partial<AiPoint> = {}): AiPoint {
  return {
    point_index: 0,
    name: '',
    event_class: 'Class2',
    minimum: 0,
    maximum: 0,
    multiplier: 1,
    offset: 0,
    units: '',
    iec_61850_uid: '',
    value: 0,
    purpose: '',
    mandatory_1547: false,
    mandatory_1815: false,
    ...overrides,
  }
}

function makeSchedule(
  opts: {
    timeOffsets?: AiPoint[]
    actionTypes?: AiPoint[]
    actionIndexes?: AiPoint[]
    values?: AiPoint[]
    numberOfPoints?: number
  } = {},
): AiSchedule {
  return {
    identity: makePoint({ value: 0 }),
    priority: makePoint({ value: 0 }),
    start_date: makePoint({ value: 0 }),
    start_time: makePoint({ value: 0 }),
    stop_date: makePoint({ value: 0 }),
    stop_time: makePoint({ value: 0 }),
    repeat_interval: makePoint({ value: 0 }),
    repeat_interval_units: makePoint({ value: 0 }),
    number_of_points: makePoint({ value: opts.numberOfPoints ?? 0 }),
    status: makePoint({ value: 0 }),
    validation_state: makePoint({ value: 0 }),
    time_offsets: opts.timeOffsets ?? [],
    action_types: opts.actionTypes ?? [],
    action_indexes: opts.actionIndexes ?? [],
    values: opts.values ?? [],
  }
}

function makeMinimalProfile(schedule: AiSchedule): PicsProfile {
  // We only care about AI.schedules for setSchedulePoints; the helper does
  // a structuredClone of the entire profile, so the rest must be cloneable
  // but is otherwise unused. We cast through unknown to keep the fixture
  // small without hand-rolling every section.
  const profile = {
    AI: { schedules: [schedule] },
  } as unknown as PicsProfile
  return profile
}

// --- Regression tests -----------------------------------------------------

describe('setSchedulePoints', () => {
  it('grows empty parallel arrays when caller supplies points', () => {
    const profile = makeMinimalProfile(makeSchedule())

    const next = setSchedulePoints(profile, 0, [
      { timeOffset: 30, actionType: 1, actionIndex: 42, value: 7 },
    ])

    const after = next.AI.schedules[0]
    expect(after.time_offsets).toHaveLength(1)
    expect(after.action_types).toHaveLength(1)
    expect(after.action_indexes).toHaveLength(1)
    expect(after.values).toHaveLength(1)

    expect(after.time_offsets[0]?.value).toBe(30)
    expect(after.action_types[0]?.value).toBe(1)
    expect(after.action_indexes[0]?.value).toBe(42)
    expect(after.values[0]?.value).toBe(7)
    expect(after.number_of_points.value).toBe(1)
  })

  it('grows empty parallel arrays to several points', () => {
    const profile = makeMinimalProfile(makeSchedule())

    const next = setSchedulePoints(profile, 0, [
      { timeOffset: 0, actionType: 1, actionIndex: 10, value: 100 },
      { timeOffset: 60, actionType: 1, actionIndex: 11, value: 200 },
      { timeOffset: 120, actionType: 3, actionIndex: 0, value: 0 },
    ])

    const after = next.AI.schedules[0]
    expect(after.time_offsets).toHaveLength(3)
    expect(after.action_types).toHaveLength(3)
    expect(after.action_indexes).toHaveLength(3)
    expect(after.values).toHaveLength(3)
    expect(after.number_of_points.value).toBe(3)

    expect(after.time_offsets.map((p) => p.value)).toEqual([0, 60, 120])
    expect(after.values.map((p) => p.value)).toEqual([100, 200, 0])
  })

  it('still clamps down when parallel arrays are longer than points', () => {
    // Sanity check that the previously-working shrink path still works.
    const profile = makeMinimalProfile(
      makeSchedule({
        timeOffsets: [makePoint(), makePoint(), makePoint()],
        actionTypes: [makePoint(), makePoint(), makePoint()],
        actionIndexes: [makePoint(), makePoint(), makePoint()],
        values: [makePoint(), makePoint(), makePoint()],
        numberOfPoints: 3,
      }),
    )

    const next = setSchedulePoints(profile, 0, [
      { timeOffset: 1, actionType: 1, actionIndex: 1, value: 1 },
    ])

    const after = next.AI.schedules[0]
    expect(after.time_offsets).toHaveLength(1)
    expect(after.action_types).toHaveLength(1)
    expect(after.action_indexes).toHaveLength(1)
    expect(after.values).toHaveLength(1)
    expect(after.number_of_points.value).toBe(1)
  })

  it('handles the empty-to-empty no-op without error', () => {
    const profile = makeMinimalProfile(makeSchedule())
    const next = setSchedulePoints(profile, 0, [])
    expect(next.AI.schedules[0].time_offsets).toHaveLength(0)
    expect(next.AI.schedules[0].number_of_points.value).toBe(0)
  })

  it('returns the input untouched for an out-of-range schedule index', () => {
    const profile = makeMinimalProfile(makeSchedule())
    const next = setSchedulePoints(profile, 5, [
      { timeOffset: 0, actionType: 0, actionIndex: 0, value: 0 },
    ])
    expect(next).toBe(profile)
  })
})

// --- End-to-end against the real full.json profile -----------------------

describe('applySchedulesToProfile against full.json', () => {
  const baseProfile = fullProfileJson as unknown as PicsProfile

  it('adds a power-mode point to the freshly loaded full profile without throwing', () => {
    const profile = structuredClone(baseProfile)
    const schedules = extractSchedulesFromProfile(profile)
    // Simulate the editor's handleModeToggle "Set Active Power" path: append
    // a single action point to the first schedule.
    schedules[0] = {
      ...schedules[0],
      points: [{ timeOffset: 0, actionType: 1, actionIndex: 1024, value: 50 }],
      numberOfPoints: 1,
    }

    expect(() => applySchedulesToProfile(profile, schedules)).not.toThrow()

    const next = applySchedulesToProfile(profile, schedules)
    const after = next.AI.schedules[0]
    expect(after.number_of_points.value).toBe(1)
    expect(after.time_offsets[0]?.value).toBe(0)
    expect(after.action_types[0]?.value).toBe(1)
    expect(after.action_indexes[0]?.value).toBe(1024)
    expect(after.values[0]?.value).toBe(50)

    // point_index must match the canonical scheme: 3012/3013/3014/3015 for
    // slot 0. The original full.json had 175 slots; we're shrinking down to
    // 1, which means slot 0 reuses the existing AiPoints — their indices
    // must be preserved.
    expect(after.time_offsets[0]?.point_index).toBe(3012)
    expect(after.action_types[0]?.point_index).toBe(3013)
    expect(after.action_indexes[0]?.point_index).toBe(3014)
    expect(after.values[0]?.point_index).toBe(3015)
  })

  it('still works when the first schedule was force-stripped to empty parallel arrays', () => {
    // Defensive coverage for the exact crash shape: even if some upstream
    // path leaves the parallel arrays empty, adding points must not crash
    // AND the synthesised point_index values must follow the canonical
    // scheme derived from number_of_points.point_index.
    const profile = structuredClone(baseProfile)
    profile.AI.schedules[0].time_offsets = []
    profile.AI.schedules[0].action_types = []
    profile.AI.schedules[0].action_indexes = []
    profile.AI.schedules[0].values = []
    profile.AI.schedules[0].number_of_points.value = 0
    // Sanity: the header AiPoint is still there, carrying its real index.
    expect(profile.AI.schedules[0].number_of_points.point_index).toBe(3011)

    const schedules = extractSchedulesFromProfile(profile)
    schedules[0] = {
      ...schedules[0],
      points: [
        { timeOffset: 0, actionType: 1, actionIndex: 1024, value: 25 },
        { timeOffset: 3600, actionType: 1, actionIndex: 1024, value: 75 },
      ],
      numberOfPoints: 2,
    }

    expect(() => applySchedulesToProfile(profile, schedules)).not.toThrow()
    const next = applySchedulesToProfile(profile, schedules)
    expect(next.AI.schedules[0].time_offsets.map((p) => p.value)).toEqual([
      0, 3600,
    ])
    expect(next.AI.schedules[0].values.map((p) => p.value)).toEqual([25, 75])

    // The point_index values must be derived from number_of_points.point_index
    // (3011) + 1 + arrayOrdinal + 4*slot — NOT 0, NOT cloned from a sibling.
    expect(next.AI.schedules[0].time_offsets.map((p) => p.point_index)).toEqual(
      [3012, 3016],
    )
    expect(next.AI.schedules[0].action_types.map((p) => p.point_index)).toEqual(
      [3013, 3017],
    )
    expect(
      next.AI.schedules[0].action_indexes.map((p) => p.point_index),
    ).toEqual([3014, 3018])
    expect(next.AI.schedules[0].values.map((p) => p.point_index)).toEqual([
      3015, 3019,
    ])
  })
})

// --- point_index correctness on grow paths (Copilot review findings) ------
//
// These tests guard against the regression where the fix for #327 silently
// produced an invalid profile by synthesising points with point_index = 0 or
// by cloning across parallel arrays (e.g. growing time_offsets from an
// action_types template, which carries the WRONG base index).

describe('setSchedulePoints point_index correctness', () => {
  // Indices follow the canonical schedule layout: 11 header AiPoints
  // (last one is `number_of_points`), then time_offsets / action_types /
  // action_indexes / values interleaved with stride 4.
  const HEADER_LAST_INDEX = 3011 // matches full.json schedule 0
  const TIME_OFFSETS_BASE = HEADER_LAST_INDEX + 1 // 3012
  const ACTION_TYPES_BASE = HEADER_LAST_INDEX + 2 // 3013
  const ACTION_INDEXES_BASE = HEADER_LAST_INDEX + 3 // 3014
  const VALUES_BASE = HEADER_LAST_INDEX + 4 // 3015
  const SLOT_STRIDE = 4

  function makeIndexedSchedule(
    opts: {
      headerLastIndex?: number
      timeOffsets?: AiPoint[]
      actionTypes?: AiPoint[]
      actionIndexes?: AiPoint[]
      values?: AiPoint[]
    } = {},
  ): AiSchedule {
    const lastHeader = opts.headerLastIndex ?? HEADER_LAST_INDEX
    // Header AiPoints carry their canonical indices so derivation has a hook
    // even when every parallel array is empty.
    return {
      identity: makePoint({ point_index: lastHeader - 10 }),
      priority: makePoint({ point_index: lastHeader - 9 }),
      start_date: makePoint({ point_index: lastHeader - 8 }),
      start_time: makePoint({ point_index: lastHeader - 7 }),
      stop_date: makePoint({ point_index: lastHeader - 6 }),
      stop_time: makePoint({ point_index: lastHeader - 5 }),
      repeat_interval: makePoint({ point_index: lastHeader - 4 }),
      repeat_interval_units: makePoint({ point_index: lastHeader - 3 }),
      validation_state: makePoint({ point_index: lastHeader - 2 }),
      status: makePoint({ point_index: lastHeader - 1 }),
      number_of_points: makePoint({ point_index: lastHeader }),
      time_offsets: opts.timeOffsets ?? [],
      action_types: opts.actionTypes ?? [],
      action_indexes: opts.actionIndexes ?? [],
      values: opts.values ?? [],
    }
  }

  function makeProfile(sched: AiSchedule): PicsProfile {
    return { AI: { schedules: [sched] } } as unknown as PicsProfile
  }

  it('preserves existing point_index values when shrinking', () => {
    // When the array already has the slots we want, we must not rewrite their
    // point_index values.
    const profile = makeProfile(
      makeIndexedSchedule({
        timeOffsets: [
          makePoint({ point_index: TIME_OFFSETS_BASE + 0 * SLOT_STRIDE }),
          makePoint({ point_index: TIME_OFFSETS_BASE + 1 * SLOT_STRIDE }),
          makePoint({ point_index: TIME_OFFSETS_BASE + 2 * SLOT_STRIDE }),
        ],
        actionTypes: [
          makePoint({ point_index: ACTION_TYPES_BASE + 0 * SLOT_STRIDE }),
          makePoint({ point_index: ACTION_TYPES_BASE + 1 * SLOT_STRIDE }),
          makePoint({ point_index: ACTION_TYPES_BASE + 2 * SLOT_STRIDE }),
        ],
        actionIndexes: [
          makePoint({ point_index: ACTION_INDEXES_BASE + 0 * SLOT_STRIDE }),
          makePoint({ point_index: ACTION_INDEXES_BASE + 1 * SLOT_STRIDE }),
          makePoint({ point_index: ACTION_INDEXES_BASE + 2 * SLOT_STRIDE }),
        ],
        values: [
          makePoint({ point_index: VALUES_BASE + 0 * SLOT_STRIDE }),
          makePoint({ point_index: VALUES_BASE + 1 * SLOT_STRIDE }),
          makePoint({ point_index: VALUES_BASE + 2 * SLOT_STRIDE }),
        ],
      }),
    )

    const next = setSchedulePoints(profile, 0, [
      { timeOffset: 1, actionType: 1, actionIndex: 1, value: 1 },
    ])
    const after = next.AI.schedules[0]
    expect(after.time_offsets[0].point_index).toBe(3012)
    expect(after.action_types[0].point_index).toBe(3013)
    expect(after.action_indexes[0].point_index).toBe(3014)
    expect(after.values[0].point_index).toBe(3015)
  })

  it('derives point_index for grown slots from the canonical base when the array has one entry', () => {
    // time_offsets has slot 0 only (idx 3012). Grow to 3 slots -> new slots
    // must be idx 3016 and 3020, not cloned-from-3012-with-still-3012.
    const profile = makeProfile(
      makeIndexedSchedule({
        timeOffsets: [makePoint({ point_index: TIME_OFFSETS_BASE })],
        actionTypes: [makePoint({ point_index: ACTION_TYPES_BASE })],
        actionIndexes: [makePoint({ point_index: ACTION_INDEXES_BASE })],
        values: [makePoint({ point_index: VALUES_BASE })],
      }),
    )

    const next = setSchedulePoints(profile, 0, [
      { timeOffset: 0, actionType: 0, actionIndex: 0, value: 0 },
      { timeOffset: 1, actionType: 1, actionIndex: 1, value: 1 },
      { timeOffset: 2, actionType: 2, actionIndex: 2, value: 2 },
    ])
    const after = next.AI.schedules[0]
    expect(after.time_offsets.map((p) => p.point_index)).toEqual([
      3012, 3016, 3020,
    ])
    expect(after.action_types.map((p) => p.point_index)).toEqual([
      3013, 3017, 3021,
    ])
    expect(after.action_indexes.map((p) => p.point_index)).toEqual([
      3014, 3018, 3022,
    ])
    expect(after.values.map((p) => p.point_index)).toEqual([3015, 3019, 3023])
  })

  it("uses each array's OWN base when growing an empty array beside a non-empty sibling", () => {
    // time_offsets is empty; the siblings are all non-empty with EXPLICITLY
    // WRONG point_index values (9999...) so a sibling-clone fallback would
    // produce 9999. Correctness requires deriving the time_offsets base from
    // the schedule's number_of_points.point_index, NOT from the siblings.
    const profile = makeProfile(
      makeIndexedSchedule({
        timeOffsets: [],
        actionTypes: [makePoint({ point_index: 9999 })],
        actionIndexes: [makePoint({ point_index: 9999 })],
        values: [makePoint({ point_index: 9999 })],
      }),
    )

    const next = setSchedulePoints(profile, 0, [
      { timeOffset: 30, actionType: 1, actionIndex: 42, value: 7 },
      { timeOffset: 60, actionType: 1, actionIndex: 42, value: 14 },
    ])
    const after = next.AI.schedules[0]
    // time_offsets MUST derive from 3011 + 1 = 3012 (NOT 9999, NOT 0).
    expect(after.time_offsets.map((p) => p.point_index)).toEqual([3012, 3016])
    // Siblings preserve their existing slot 0 even if its point_index is bogus
    // (we don't rewrite the data we already have) AND derive slot 1 from base.
    expect(after.action_types[1].point_index).toBe(3017)
    expect(after.action_indexes[1].point_index).toBe(3018)
    expect(after.values[1].point_index).toBe(3019)
  })

  it('derives all four bases from number_of_points.point_index when ALL arrays are empty', () => {
    // The crash-scenario shape: every parallel array empty. The only data we
    // have to anchor on is the header's number_of_points.point_index. The
    // canonical layout puts time_offsets immediately after it.
    const profile = makeProfile(makeIndexedSchedule())

    const next = setSchedulePoints(profile, 0, [
      { timeOffset: 0, actionType: 1, actionIndex: 100, value: 50 },
      { timeOffset: 60, actionType: 1, actionIndex: 100, value: 75 },
    ])
    const after = next.AI.schedules[0]
    expect(after.time_offsets.map((p) => p.point_index)).toEqual([3012, 3016])
    expect(after.action_types.map((p) => p.point_index)).toEqual([3013, 3017])
    expect(after.action_indexes.map((p) => p.point_index)).toEqual([3014, 3018])
    expect(after.values.map((p) => p.point_index)).toEqual([3015, 3019])

    // And not the 0 sentinel that the old emptyAiPoint produced.
    for (const arr of [
      after.time_offsets,
      after.action_types,
      after.action_indexes,
      after.values,
    ]) {
      for (const p of arr) {
        expect(p.point_index).not.toBe(0)
      }
    }
  })

  it('honours a different schedule header position (not just 3011)', () => {
    // Sanity: if number_of_points.point_index is somewhere else (e.g. a
    // hypothetical second schedule at 3175), the same derivation still works.
    const profile = makeProfile(makeIndexedSchedule({ headerLastIndex: 3175 }))

    const next = setSchedulePoints(profile, 0, [
      { timeOffset: 0, actionType: 0, actionIndex: 0, value: 0 },
      { timeOffset: 1, actionType: 1, actionIndex: 1, value: 1 },
    ])
    const after = next.AI.schedules[0]
    expect(after.time_offsets.map((p) => p.point_index)).toEqual([3176, 3180])
    expect(after.action_types.map((p) => p.point_index)).toEqual([3177, 3181])
    expect(after.action_indexes.map((p) => p.point_index)).toEqual([3178, 3182])
    expect(after.values.map((p) => p.point_index)).toEqual([3179, 3183])
  })
})
