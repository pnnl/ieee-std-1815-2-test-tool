import type { PicsProfile } from '@/api/generated'
import { flattenSection, type PointSection } from '@/profile/canonical'
import type { ConformanceCheck, ConformanceTestResult } from './types'

export const POINT_CHECKS = [
  { check: 'written_to_outstation', label: 'Written to outstation' },
  { check: 'received_from_outstation', label: 'Received from outstation' },
  { check: 'within_profile_range', label: 'Within profile range' },
  {
    check: 'curve_readback_matches_profile',
    label: 'Curve readback matches profile',
  },
  {
    check: 'schedule_readback_matches_profile',
    label: 'Schedule readback matches profile',
  },
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
  curveReadbackApplicable: boolean
  scheduleReadbackApplicable: boolean
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
      let row = rows.get(index)
      if (!row) {
        row = {
          index,
          name: point.name,
          checks: {},
          curveReadbackApplicable: false,
          scheduleReadbackApplicable: false,
        }
        rows.set(index, row)
      }
      if (point.kind === 'ai') {
        if (point.locator.group.startsWith('curve_')) {
          row.curveReadbackApplicable = true
        }
        if (
          point.locator.group.startsWith('schedule_') &&
          point.assoc_ao != null
        ) {
          row.scheduleReadbackApplicable = true
        }
      }
    }
  }

  const analogInputs = flattenSection(profile, 'analog_inputs').filter(
    (point) => point.kind === 'ai',
  )
  const analogOutputs = flattenSection(profile, 'analog_outputs').filter(
    (point) => point.kind === 'ao',
  )
  const outputsByIndex = new Map(
    analogOutputs.map((point) => [`AO${point.point_index}`, point]),
  )

  function mergePair(inputIndex: string, outputIndex: string) {
    const inputRow = rows.get(inputIndex)
    const outputRow = rows.get(outputIndex)
    if (
      !inputRow ||
      !outputRow ||
      inputRow.index !== inputIndex ||
      outputRow.index !== outputIndex
    ) {
      return
    }
    inputRow.index = `${outputIndex} / ${inputIndex}`
    rows.set(outputIndex, inputRow)
  }

  for (const point of analogOutputs) {
    if (point.assoc_ai?.startsWith('AI')) {
      mergePair(point.assoc_ai, `AO${point.point_index}`)
    }
  }
  for (const point of analogInputs) {
    if (!point.assoc_ao?.startsWith('AO')) continue
    const output = outputsByIndex.get(point.assoc_ao)
    if (output && output.assoc_ai == null) {
      mergePair(`AI${point.point_index}`, point.assoc_ao)
    }
  }

  const binaryInputs = flattenSection(profile, 'binary_inputs').filter(
    (point) => point.kind === 'bi',
  )
  const binaryOutputs = flattenSection(profile, 'binary_outputs').filter(
    (point) => point.kind === 'bo',
  )
  const binaryOutputsByIndex = new Map(
    binaryOutputs.map((point) => [`BO${point.point_index}`, point]),
  )
  for (const point of binaryOutputs) {
    if (point.assoc_bi?.startsWith('BI')) {
      mergePair(point.assoc_bi, `BO${point.point_index}`)
    }
  }
  for (const point of binaryInputs) {
    if (!point.assoc_bo?.startsWith('BO')) continue
    const output = binaryOutputsByIndex.get(point.assoc_bo)
    if (output && output.assoc_bi == null) {
      mergePair(`BI${point.point_index}`, point.assoc_bo)
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
        messages: [
          ...(previous?.messages ?? []),
          row.index === comment.index
            ? comment.message
            : `${comment.index}: ${comment.message}`,
        ],
      }
    }
  }

  return [...new Set(rows.values())]
}
