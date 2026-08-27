// Tests for the group-level validation error badges. `groupErrorCountsByOffset`
// reuses the exact band-boundary lookup `groupPointsByOffset` uses for
// points, so the two can never drift apart.

import { describe, it, expect } from 'vitest'
import type { FlatAiPoint } from './canonical'
import {
  groupErrorCountsByOffset,
  groupErrorsByPointIndex,
  groupPointsByOffset,
} from './offsetBands'
import { err } from './testHelpers'

function point(pointIndex: number): FlatAiPoint {
  return {
    kind: 'ai',
    locator: { kind: 'ai', group: 'points', index: pointIndex },
    point_index: pointIndex,
    name: `Point ${pointIndex}`,
    purpose: '',
    units: '',
    value: 0,
    minimum: 0,
    maximum: 100,
    multiplier: 1,
    offset: 0,
    mandatory_1547: false,
    mandatory_1815: false,
    iec_61850_uid: '',
    event_class: 'None',
  }
}

describe('groupPointsByOffset', () => {
  it('assigns each point to the same band groupErrorCountsByOffset would assign its index to', () => {
    const points = [
      point(0),
      point(600),
      point(2100),
      point(12000),
      point(55000),
    ]
    const grouped = groupPointsByOffset(points)

    expect(grouped.scada.map((p) => p.point_index)).toEqual([0])
    expect(grouped.gap_1.map((p) => p.point_index)).toEqual([600])
    expect(grouped.schedules_v1.map((p) => p.point_index)).toEqual([2100])
    expect(grouped.historical_ders.map((p) => p.point_index)).toEqual([12000])
    expect(grouped.vendor.map((p) => p.point_index)).toEqual([55000])

    // Nothing dropped, nothing duplicated across bands.
    const total = Object.values(grouped).reduce((s, arr) => s + arr.length, 0)
    expect(total).toBe(points.length)
  })
})

describe('groupErrorCountsByOffset', () => {
  it('counts a single error into its band and leaves every other band at zero', () => {
    const counts = groupErrorCountsByOffset([err('AI0')])
    expect(counts.scada).toBe(1)
    expect(counts.gap_1).toBe(0)
    expect(counts.schedules_v1).toBe(0)
    expect(counts.vendor).toBe(0)
  })

  // Real errors from invalid_ai.json, filtered to the 6 that route to the
  // Analog Inputs tab; the other 4 are curve-structural and land in
  // General. All six indexes fall under the `gap_1` threshold (586), so
  // this fixture doesn't exercise a multi-band split; see the synthetic
  // case below for that.
  it('routes the real invalid_ai.json Analog Inputs errors: all six into scada', () => {
    const realErrors = [
      err('AI333'),
      err('AI0'),
      err('AI2', 'first AI2 message'),
      err('AI2', 'second AI2 message'),
      err('AI3'),
      err('AI330'),
    ]

    const counts = groupErrorCountsByOffset(realErrors)

    expect(counts.scada).toBe(6)
    expect(counts.gap_1).toBe(0)
    expect(counts.schedules_v1).toBe(0)
    expect(counts.status_v1).toBe(0)
    expect(counts.schedules_v2).toBe(0)
    expect(counts.status_v2).toBe(0)
    expect(counts.gap_2).toBe(0)
    expect(counts.historical_meters).toBe(0)
    expect(counts.historical_ders).toBe(0)
    expect(counts.historical_inverters).toBe(0)
    expect(counts.historical_batteries).toBe(0)
    expect(counts.gap_3).toBe(0)
    expect(counts.vendor).toBe(0)

    // Nesting invariant: sum of every group's count equals the tab total.
    const sum = Object.values(counts).reduce((a, b) => a + b, 0)
    expect(sum).toBe(realErrors.length)
    expect(sum).toBe(6)
  })

  // Synthetic multi-band case: one error placed in each of five distinct
  // bands, verified against `SECTION_OFFSETS`'s real thresholds
  // (canonical.ts:65-78) so the test fails if a threshold ever moves.
  it('splits errors across bands correctly and sums to the total (multi-band case)', () => {
    const multiband = [
      err('AI0'), // scada:            0 <= 0    < 586
      err('AI600'), // gap_1:          586 <= 600  < 2000
      err('AI2100'), // schedules_v1: 2000 <= 2100 < 2300
      err('AI12000'), // historical_ders: 10000 <= 12000 < 15000
      err('AI55000'), // vendor:      50000 <= 55000
    ]

    const counts = groupErrorCountsByOffset(multiband)

    expect(counts.scada).toBe(1)
    expect(counts.gap_1).toBe(1)
    expect(counts.schedules_v1).toBe(1)
    expect(counts.historical_ders).toBe(1)
    expect(counts.vendor).toBe(1)

    const otherBands = [
      'status_v1',
      'schedules_v2',
      'status_v2',
      'gap_2',
      'historical_meters',
      'historical_inverters',
      'historical_batteries',
      'gap_3',
    ]
    for (const band of otherBands) {
      expect(counts[band]).toBe(0)
    }

    const sum = Object.values(counts).reduce((a, b) => a + b, 0)
    expect(sum).toBe(multiband.length)
  })

  it('drops nothing and double-counts nothing across a mixed multi-band input', () => {
    const errors = [
      err('AI5', 'a'),
      err('AI5', 'b'),
      err('AI700', 'c'),
      err('AI2050', 'd'),
      err('AI50001', 'e'),
    ]
    const counts = groupErrorCountsByOffset(errors)
    const sum = Object.values(counts).reduce((a, b) => a + b, 0)
    expect(sum).toBe(errors.length)
  })

  it('does not count an error whose point does not resolve to a single point index', () => {
    // This case should never reach groupErrorCountsByOffset in practice
    // (bucketValidationErrors already routed it to General), but the
    // function's own contract is: uncounted, not force-assigned to scada.
    const counts = groupErrorCountsByOffset([err('AI12 and AI13'), err('AI0')])
    const sum = Object.values(counts).reduce((a, b) => a + b, 0)
    expect(sum).toBe(1)
    expect(counts.scada).toBe(1)
  })

  it('returns every band key at zero for an empty input', () => {
    const counts = groupErrorCountsByOffset([])
    const sum = Object.values(counts).reduce((a, b) => a + b, 0)
    expect(sum).toBe(0)
  })
})

// Row-highlighting lookup: a point with two errors gets ONE highlighted
// row. `groupErrorCountsByOffset` counts ERRORS (6), `groupErrorsByPointIndex`
// counts ERRORED POINTS (5); the two numbers are not expected to match.
describe('groupErrorsByPointIndex', () => {
  it('maps the real invalid_ai.json Analog Inputs errors: 6 errors resolve to 5 distinct points', () => {
    const realErrors = [
      err(
        'AI333',
        'Expected minimum 0 per curve scaling table, got TransmissionI32(5)',
      ),
      err('AI0', 'Value must be between 1 and 100, but is 999'),
      err('AI2', 'Minimum value cannot be greater than maximum value'),
      err('AI2', 'Value must be between 10 and 0, but is 0'),
      err('AI3', 'Multiplier cannot be zero'),
      err('AI330', 'Value must be between 0 and 100, but is 500'),
    ]

    const byPoint = groupErrorsByPointIndex(realErrors)

    // Distinct points: 0, 2, 3, 330, 333 -- five, not six.
    expect(byPoint.size).toBe(5)
    expect([...byPoint.keys()].sort((a, b) => a - b)).toEqual([
      0, 2, 3, 330, 333,
    ])

    // AI2 alone carries both of its errors, verbatim.
    expect(byPoint.get(2)).toEqual([
      err('AI2', 'Minimum value cannot be greater than maximum value'),
      err('AI2', 'Value must be between 10 and 0, but is 0'),
    ])
    // Every other errored point carries exactly its one error.
    expect(byPoint.get(0)).toHaveLength(1)
    expect(byPoint.get(3)).toHaveLength(1)
    expect(byPoint.get(330)).toHaveLength(1)
    expect(byPoint.get(333)).toHaveLength(1)

    // Sum of per-point error counts equals the total error count: the
    // errors-side invariant, a DIFFERENT number than byPoint.size (5 points).
    const errorSum = [...byPoint.values()].reduce((s, arr) => s + arr.length, 0)
    expect(errorSum).toBe(realErrors.length)
    expect(errorSum).toBe(6)
    expect(byPoint.size).toBe(5)

    // The per-band error count (badge) and the per-band errored-point count
    // (rows) both exist for `scada` and are NOT the same number.
    const bandCounts = groupErrorCountsByOffset(realErrors)
    expect(bandCounts.scada).toBe(6)
    expect(byPoint.size).toBe(5)
  })

  it('does not map an error whose point does not resolve to a single point index', () => {
    const byPoint = groupErrorsByPointIndex([err('AI12 and AI13'), err('AI0')])
    expect(byPoint.size).toBe(1)
    expect(byPoint.get(0)).toHaveLength(1)
  })

  it('returns an empty map for an empty input', () => {
    expect(groupErrorsByPointIndex([]).size).toBe(0)
  })
})
