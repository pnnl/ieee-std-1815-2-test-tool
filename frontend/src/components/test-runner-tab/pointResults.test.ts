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
    expect(
      indices.some((index) =>
        index
          .split(' / ')
          .includes(`AI${profile.AI.curves[0].curve_type.point_index}`),
      ),
    ).toBe(true)
    expect(
      indices.some((index) =>
        index.split(' / ').includes(`BI${profile.BI.points[0].point_index}`),
      ),
    ).toBe(true)
    expect(
      indices.some((index) =>
        index.split(' / ').includes(`AO${profile.AO.points[0].point_index}`),
      ),
    ).toBe(true)
    expect(
      indices.some((index) =>
        index.split(' / ').includes(`BO${profile.BO.points[0].point_index}`),
      ),
    ).toBe(true)
    expect(rows.every((row) => Object.keys(row.checks).length === 0)).toBe(true)
  })

  it('merges associated analog points and combines their results', () => {
    const firstAi = profile.AI.points[0]
    const secondAi = profile.AI.points[1]
    const firstAo = profile.AO.points[0]
    const secondAo = profile.AO.points[1]
    const testProfile = structuredClone(profile)
    testProfile.AI.points = [
      { ...firstAi, point_index: 60001, assoc_ao: 'AO60002' },
      { ...secondAi, point_index: 60003, assoc_ao: 'AO60004' },
    ]
    testProfile.AO.points = [
      { ...firstAo, point_index: 60002, assoc_ai: 'AI60001' },
      { ...secondAo, point_index: 60004, assoc_ai: null },
    ]
    const comment: SunSpecComment = {
      uid: 'uid',
      index: 'AI60001',
      check: 'within_profile_range',
      value: '',
      message: 'In range',
      passed: true,
    }
    const rows = buildPointResults(testProfile, {
      test: result([
        comment,
        {
          ...comment,
          index: 'AO60002',
          message: 'Out of range',
          passed: false,
        },
        { ...comment, index: 'AO60004', check: 'written_to_outstation' },
      ]),
    })
    const firstRow = rows.find((row) => row.index === 'AO60002 / AI60001')
    expect(firstRow?.name).toBe(firstAi.name)
    expect(firstRow?.checks.within_profile_range).toEqual({
      passed: false,
      messages: ['AI60001: In range', 'AO60002: Out of range'],
    })
    expect(
      rows.find((row) => row.index === 'AO60004 / AI60003')?.checks
        .written_to_outstation?.passed,
    ).toBe(true)
    expect(
      rows.some((row) => row.index === 'AI60001' || row.index === 'AO60002'),
    ).toBe(false)
  })

  it('merges associated binary points and combines their results', () => {
    const testProfile = structuredClone(profile)
    testProfile.BI.points = [
      { ...profile.BI.points[0], point_index: 60001, assoc_bo: 'BO60002' },
      { ...profile.BI.points[1], point_index: 60003, assoc_bo: 'BO60004' },
      { ...profile.BI.points[2], point_index: 60005, assoc_bo: 'BO60006' },
      { ...profile.BI.points[3], point_index: 60007, assoc_bo: null },
      { ...profile.BI.points[4], point_index: 60008, assoc_bo: 'BO60009' },
    ]
    testProfile.BO.points = [
      { ...profile.BO.points[0], point_index: 60002, assoc_bi: 'BI60001' },
      { ...profile.BO.points[1], point_index: 60004, assoc_bi: null },
      { ...profile.BO.points[2], point_index: 60006, assoc_bi: 'BI60007' },
      { ...profile.BO.points[3], point_index: 60010, assoc_bi: 'BI60011' },
    ]
    const comment: SunSpecComment = {
      uid: 'uid',
      index: 'BI60001',
      check: 'received_from_outstation',
      value: '',
      message: 'Received',
      passed: true,
    }
    const rows = buildPointResults(testProfile, {
      test: result([
        comment,
        {
          ...comment,
          index: 'BO60002',
          check: 'written_to_outstation',
          message: 'Not written',
          passed: false,
        },
        { ...comment, index: 'BO60004', check: 'written_to_outstation' },
      ]),
    })
    const row = rows.find((item) => item.index === 'BO60002 / BI60001')
    expect(row?.name).toBe(profile.BI.points[0].name)
    expect(row?.checks.received_from_outstation).toEqual({
      passed: true,
      messages: ['BI60001: Received'],
    })
    expect(row?.checks.written_to_outstation).toEqual({
      passed: false,
      messages: ['BO60002: Not written'],
    })
    expect(
      rows.find((item) => item.index === 'BO60004 / BI60003')?.checks
        .written_to_outstation?.passed,
    ).toBe(true)
    expect(
      rows.some((item) => item.index === 'BO60002' || item.index === 'BI60001'),
    ).toBe(false)
    expect(rows.some((item) => item.index === 'BO60006 / BI60007')).toBe(true)
    expect(rows.some((item) => item.index === 'BI60005')).toBe(true)
    expect(rows.some((item) => item.index === 'BI60008')).toBe(true)
    expect(rows.some((item) => item.index === 'BO60010')).toBe(true)
  })

  it('keeps points separate when an association is missing or conflicts', () => {
    const testProfile = structuredClone(profile)
    testProfile.AI.points = [
      { ...profile.AI.points[0], point_index: 60001, assoc_ao: 'AO60002' },
      { ...profile.AI.points[1], point_index: 60003, assoc_ao: 'AO60004' },
      { ...profile.AI.points[2], point_index: 60005, assoc_ao: null },
    ]
    testProfile.AO.points = [
      { ...profile.AO.points[0], point_index: 60004, assoc_ai: 'AI60005' },
      { ...profile.AO.points[1], point_index: 60006, assoc_ai: 'AI60007' },
    ]
    const indices = buildPointResults(testProfile, {}).map((row) => row.index)
    expect(indices).toContain('AI60001')
    expect(indices).toContain('AI60003')
    expect(indices).toContain('AO60004 / AI60005')
    expect(indices).toContain('AO60006')
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

  it('associates curve and schedule comparisons with point rows', () => {
    const index = `AI${profile.AI.curves[0].curve_type.point_index}`
    const comment: SunSpecComment = {
      uid: 'uid',
      index,
      check: 'curve_readback_matches_profile',
      value: '',
      message: 'Curve 1 matches',
      passed: true,
    }
    const rows = buildPointResults(profile, {
      first: result([
        comment,
        {
          ...comment,
          check: 'schedule_readback_matches_profile',
          message: 'Schedule 1 differs',
          passed: false,
        },
        { ...comment, index: 'Curve 2', message: 'No readback', passed: false },
      ]),
      second: result([
        { ...comment, message: 'Curve 2 differs', passed: false },
      ]),
    })
    const row = rows.find((item) => item.index.split(' / ').includes(index))
    expect(row?.checks.curve_readback_matches_profile).toEqual({
      passed: false,
      messages: [`${index}: Curve 1 matches`, `${index}: Curve 2 differs`],
    })
    expect(row?.checks.schedule_readback_matches_profile).toEqual({
      passed: false,
      messages: [`${index}: Schedule 1 differs`],
    })
  })
})
