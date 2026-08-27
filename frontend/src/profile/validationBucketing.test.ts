// Tests for the point-to-tab bucketing rules. Each non-mappable case is
// asserted individually, plus the no-drop/no-duplicate obligations.

import { describe, it, expect } from 'vitest'
import type { ValidationError } from '@/api/generated'
import { bucketValidationErrors } from './validationBucketing'
import { err } from './testHelpers'

describe('bucketValidationErrors', () => {
  it('routes AO<digits> to analog_outputs', () => {
    const input = [err('AO245')]
    expect(bucketValidationErrors(input).analog_outputs).toEqual(input)
  })

  it('routes AI<digits> to analog_inputs', () => {
    const input = [err('AI12')]
    expect(bucketValidationErrors(input).analog_inputs).toEqual(input)
  })

  it('routes BO<digits> to binary_outputs', () => {
    const input = [err('BO3')]
    expect(bucketValidationErrors(input).binary_outputs).toEqual(input)
  })

  it('routes BI<digits> to binary_inputs', () => {
    const input = [err('BI7')]
    expect(bucketValidationErrors(input).binary_inputs).toEqual(input)
  })

  it.each([
    ['composite point list', 'AI12 and AI13'],
    ['non-digit AI-prefixed enum name', 'AiEnum'],
    ['structural workbook fault literal', 'N/A'],
    ['missing Key sheet literal', 'Key'],
    ['bare numeric load-fault index', '253'],
    ['empty point string', ''],
  ])('routes %s ("%s") to general', (_label, point) => {
    const input = [err(point)]
    const buckets = bucketValidationErrors(input)
    expect(buckets.general).toEqual(input)
    expect(buckets.analog_inputs).toEqual([])
    expect(buckets.analog_outputs).toEqual([])
    expect(buckets.binary_inputs).toEqual([])
    expect(buckets.binary_outputs).toEqual([])
    expect(buckets.curves).toEqual([])
    expect(buckets.scheduling).toEqual([])
  })

  it.each([
    [
      'curve structure display name',
      "<Curve of type 'Volt-Var' (4 points, x_units: V, y_units: VAr)>: curve type (AI42)",
    ],
  ])(
    'routes %s ("%s") to curves, not general or analog_inputs',
    (_label, point) => {
      const input = [err(point)]
      const buckets = bucketValidationErrors(input)
      expect(buckets.curves).toEqual(input)
      expect(buckets.general).toEqual([])
      expect(buckets.analog_inputs).toEqual([])
    },
  )

  it.each([
    ['schedule display name', 'Schedule 3'],
    ['schedule field path', 'Schedule 3.time_offsets[2]'],
  ])('routes %s ("%s") to scheduling, not general', (_label, point) => {
    const input = [err(point)]
    const buckets = bucketValidationErrors(input)
    expect(buckets.scheduling).toEqual(input)
    expect(buckets.general).toEqual([])
  })

  it('does not throw on the empty point string', () => {
    expect(() => bucketValidationErrors([err('')])).not.toThrow()
  })

  it('conserves every error across a mixed input: no drops, no duplicates', () => {
    const input: ValidationError[] = [
      err('AO245'),
      err('AI12'),
      err('BO3'),
      err('BI7'),
      err('AI12 and AI13'),
      err('AiEnum'),
      err(
        "<Curve of type 'Volt-Var' (4 points, x_units: V, y_units: VAr)>: curve type (AI42)",
      ),
      err('Schedule 3'),
      err('Schedule 3.time_offsets[2]'),
      err('N/A'),
      err('Key'),
      err('253'),
      err(''),
    ]

    const buckets = bucketValidationErrors(input)
    const combined = [
      ...buckets.analog_outputs,
      ...buckets.analog_inputs,
      ...buckets.binary_outputs,
      ...buckets.binary_inputs,
      ...buckets.curves,
      ...buckets.scheduling,
      ...buckets.general,
    ]

    expect(combined).toHaveLength(input.length)
    expect(combined.map((e) => e.message).sort()).toEqual(
      input.map((e) => e.message).sort(),
    )
  })

  // Guards the false-positive a message-based "contains 'curve'" sniff
  // would create: an AI-scaling error's message can mention "curve" while
  // its `point` is a bare `AI<digits>`, and must stay on Analog Inputs.
  it('keeps an AI-scaling error whose message mentions "curve" on analog_inputs, not curves', () => {
    const input = [
      err(
        'AI333',
        'Expected minimum 0 per curve scaling table, got TransmissionI32(5)',
      ),
    ]
    const buckets = bucketValidationErrors(input)
    expect(buckets.analog_inputs).toEqual(input)
    expect(buckets.curves).toEqual([])
  })

  it('preserves input order within a bucket', () => {
    const input = [
      err('AI1', 'first'),
      err('AI2', 'second'),
      err('AI3', 'third'),
    ]
    expect(
      bucketValidationErrors(input).analog_inputs.map((e) => e.message),
    ).toEqual(['first', 'second', 'third'])
  })

  // Real errors from `POST /api/profiles/validate` against
  // `invalid_general.json`: the three curve-structural messages route to
  // Curves, the schedule message to Scheduling, and the two-point composite
  // ("AI5026 and AI5025", which fails the full-string match) to General.
  it('routes the real invalid_general.json backend errors: curve-structural to Curves, schedule to Scheduling, composite to General', () => {
    const input = [
      err(
        "<Curve of type 'Unknown' (4 points, x_units: TimeMs, y_units: VoltsPctVRef)>: curve type (AI329)",
        'Invalid curve type 99, expected a whole number between 0 and 16',
      ),
      err(
        "<Curve of type 'Unknown' (4 points, x_units: TimeMs, y_units: VoltsPctVRef)>: x_units (AI331)",
        'Curve type Unknown is not compatible with x_units TimeMs. Compatible units: [NotDefined]',
      ),
      err(
        "<Curve of type 'Unknown' (4 points, x_units: TimeMs, y_units: VoltsPctVRef)>: y_units (AI332)",
        'Curve type Unknown is not compatible with y_units VoltsPctVRef. Compatible units: [NotApplicable]',
      ),
      err(
        'Schedule 1',
        'Time offsets, action types, action indexes, and values must have the same length. Lengths are time_offsets: 100, action_types: 100, action_indexes: 100, values: 100, number_of_points: 999',
      ),
      err(
        'AI5026 and AI5025',
        'Low threshold (100) is greater than high threshold (50)',
      ),
    ]

    const buckets = bucketValidationErrors(input)

    expect(buckets.curves).toEqual([input[0], input[1], input[2]])
    expect(buckets.scheduling).toEqual([input[3]])
    expect(buckets.general).toEqual([input[4]])
    expect(buckets.analog_inputs).toEqual([])
    expect(buckets.analog_outputs).toEqual([])
    expect(buckets.binary_inputs).toEqual([])
    expect(buckets.binary_outputs).toEqual([])

    // Nothing dropped, nothing double-counted: sum of all buckets equals
    // the backend's count for invalid_general.json.
    const total =
      buckets.general.length +
      buckets.analog_inputs.length +
      buckets.analog_outputs.length +
      buckets.binary_inputs.length +
      buckets.binary_outputs.length +
      buckets.curves.length +
      buckets.scheduling.length
    expect(total).toBe(5)
  })
})
