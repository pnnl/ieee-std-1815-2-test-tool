import type { PicsProfile } from '@/api/generated'
import { flattenSection, type PointSection } from '@/profile/canonical'
import type { ConformanceCheck, ConformanceTestResult } from './types'

export const POINT_CHECKS = [
  { check: 'received_from_outstation', label: 'Received from outstation' },
  { check: 'written_to_outstation', label: 'Written to outstation' },
  { check: 'within_profile_range', label: 'Within profile range' },
] as const satisfies ReadonlyArray<{ check: ConformanceCheck; label: string }>

type PointCheck = (typeof POINT_CHECKS)[number]['check']

export type PointResult = {
  passed: boolean
  messages: string[]
}

export type PointResultRow = {
  index: string
  name: string
  checks: Partial<Record<PointCheck, PointResult>>
}

export function buildPointResults(
  profile: PicsProfile,
  results: Record<string, ConformanceTestResult>,
): PointResultRow[] {
  const rows = new Map<string, PointResultRow>()
  const sections: PointSection[] = [
    'analog_inputs',
    'binary_inputs',
    'analog_outputs',
    'binary_outputs',
  ]

  for (const section of sections) {
    const points = flattenSection(profile, section).sort(
      (first, second) => first.point_index - second.point_index,
    )
    for (const point of points) {
      const index = `${point.kind.toUpperCase()}${point.point_index}`
      if (!rows.has(index)) {
        rows.set(index, { index, name: point.name, checks: {} })
      }
    }
  }

  for (const result of Object.values(results)) {
    for (const comment of result.comments) {
      const row = rows.get(comment.index)
      const column = POINT_CHECKS.find(({ check }) => check === comment.check)
      if (!row || !column) continue
      const previous = row.checks[column.check]
      row.checks[column.check] = {
        passed: (previous?.passed ?? true) && comment.passed,
        messages: [...(previous?.messages ?? []), comment.message],
      }
    }
  }

  return [...rows.values()]
}
