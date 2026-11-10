import { describe, expect, it } from 'vitest'
import type { PicsProfile } from '@/api/generated'
import fullProfileJson from '../../../../data/profiles/full.json'
import { buildPointResults } from './pointResults'
import type { ConformanceTestResult, SunSpecComment } from './types'

const profile = fullProfileJson as PicsProfile

function result(comments: SunSpecComment[]): ConformanceTestResult {
  return {
    event_type: 'conformance_test_result',
    scenario_id: 'scenario',
    test: 'MON_001',
    timestamp: 1,
    passed: true,
    comments,
  }
}

describe('buildPointResults', () => {
  it('initializes unique profile indices, including equipment and curve points', () => {
    const rows = buildPointResults(profile, {})
    const indices = rows.map((row) => row.index)
    expect(new Set(indices).size).toBe(indices.length)
    expect(indices).toContain(`AI${profile.AI.points[0].point_index}`)
    expect(indices).toContain(`AI${profile.AI.meters[0].frequency.point_index}`)
    expect(indices).toContain(
      `AI${profile.AI.curves[0].curve_type.point_index}`,
    )
    expect(indices).toContain(`BI${profile.BI.points[0].point_index}`)
    expect(indices).toContain(`AO${profile.AO.points[0].point_index}`)
    expect(indices).toContain(`BO${profile.BO.points[0].point_index}`)
    expect(rows.every((row) => Object.keys(row.checks).length === 0)).toBe(true)
  })

  it('combines raw results by exact point and check, with any failure taking precedence', () => {
    const index = `AI${profile.AI.points[0].point_index}`
    const comment: SunSpecComment = {
      uid: 'uid',
      index,
      check: 'received_from_outstation',
      value: '',
      message: 'Received',
      passed: true,
    }
    const rows = buildPointResults(profile, {
      first: result([comment]),
      second: result([
        { ...comment, passed: false, message: 'Not received' },
        { ...comment, index: `Curve 1 ${index}`, passed: false },
        { ...comment, index: 'AI999999', passed: false },
      ]),
    })
    const row = rows.find((item) => item.index === index)
    expect(row?.checks.received_from_outstation).toEqual({
      passed: false,
      messages: ['Received', 'Not received'],
    })
    expect(row?.checks.within_profile_range).toBeUndefined()
    expect(rows.some((item) => item.index === 'AI999999')).toBe(false)
  })
})
