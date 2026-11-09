import { describe, expect, it } from 'vitest'
import { parsePointDescription } from './parsePointDescription'
import fullProfile from '../../../data/profiles/full.json'

const examples = [
  [
    'Inverter #3 Apparent Power Calculation Method. <0> Reserved <1> Vector <2> AritMeteretic',
    ['0', '1', '2'],
  ],
  [
    'Battery Bank #1 Battery State. <0> n/a, Unknown <1> Disconnected <2> Initializing <3> Connected <4> Standby <5> SOC Protection <6> Suspending <99> Fault',
    ['0', '1', '2', '3', '4', '5', '6', '99'],
  ],
  [
    'Power Factor Sign Convention. Enumeration: <1> IEC – active power <2> IEEE – lead/lag',
    ['1', '2'],
  ],
  [
    'Reference for Reactive Power Setpoints. Selects which setpoint is active. Default is <3>. <0> Not applicable / Unknown <1> Percent of Maximum Active Power (WMax) <2> Percent of Maximum Reactive Power (VArMax) <3> Percent of Available Reactive Power (VArAvl)',
    ['0', '1', '2', '3'],
  ],
  [
    'Requested Islanded Condition. Enumeration: <0> Not Used <1> Unspecified / Autonomously Determined (see BO Enable Sensed Grid Config Detection) <2> Factory Configuration <3> Default Configuration / Comms Lost <4> Normal Grid-Connected Configuration <5> Islanded Condition 1 (small, local island) <6> Islanded Condition 2 (larger, area island) <7> Islanded Condition 3 (largest, regional island) <8> 1st Alternate Grid-Connected Configuration <9> 2nd Alternate Grid-Connected Configuration <10> 3rd Alternate Grid-Connected Configuration <11-255> Reserved for future assignment',
    ['0', '1', '2', '3', '4', '5', '6', '7', '8', '9', '10', '11-255'],
  ],
] as const

function entries(text: string) {
  return parsePointDescription(text).segments.filter((s) => s.kind === 'entry')
}

function findDescription(value: unknown, prefix: string): string | undefined {
  if (!value || typeof value !== 'object') return undefined
  if (
    'name' in value &&
    typeof value.name === 'string' &&
    value.name.startsWith(prefix)
  )
    return value.name
  for (const child of Object.values(value)) {
    const found = findDescription(child, prefix)
    if (found) return found
  }
}

describe('parsePointDescription', () => {
  it.each(examples)(
    'parses inline and multiline mappings in %s',
    (text, values) => {
      for (const source of [
        text,
        text.replace(/ </g, '\n <'),
        text.replace(/</g, '&lt;').replace(/>/g, '&gt;'),
      ]) {
        expect(entries(source).map((s) => s.value)).toEqual(values)
      }
    },
  )

  it('preserves default references, labels and source spelling', () => {
    const result = parsePointDescription(examples[3][0])
    expect(result.entryCount).toBe(4)
    expect(result.segments[0]).toEqual({
      kind: 'text',
      text: expect.stringContaining('Default is <3>.'),
    })
    expect(entries(examples[0][0])[2].label).toBe('AritMeteretic')
  })

  it('parses the real long schedule without filling omitted values', () => {
    const schedule = findDescription(fullProfile, 'Selected Schedule Type.')!
    expect(schedule).toBeTruthy()
    for (const source of [schedule, schedule.replace(/\s*\n\s*/g, ' ')]) {
      const parsed = parsePointDescription(source)
      expect(parsed.entryCount).toBe(38)
      expect(entries(source).map((s) => s.value)).toEqual([
        ...Array.from({ length: 37 }, (_, index) => String(index + 1)),
        '100',
      ])
      expect(parsed.segments).toContainEqual({
        kind: 'text',
        text: expect.stringContaining('...'),
      })
      expect(entries(source)[37].label).toContain('Vendor-specific')
    }
  })

  it('keeps interspersed headings, ellipses and unsupported tokens in order', () => {
    const parsed = parsePointDescription(
      'Mode.\n<4> Four\n\nHigh Voltage ride-through curves\n<1> One ... <234+> Other\n<11-255> Reserved',
    )
    expect(parsed.segments).toEqual([
      { kind: 'text', text: 'Mode.\n' },
      { kind: 'entry', value: '4', label: 'Four' },
      { kind: 'text', text: '\n\nHigh Voltage ride-through curves\n' },
      { kind: 'entry', value: '1', label: 'One' },
      { kind: 'text', text: ' ... <234+> Other\n' },
      { kind: 'entry', value: '11-255', label: 'Reserved' },
    ])
  })

  it.each([
    '',
    'Plain description.',
    'Default is <3>.',
    'A value <3> means three.',
    '<1>\n<2>',
    'Enumeration: <1> . <2> !',
    'A<1> One B<2> Two',
  ])('leaves ambiguous or plain text unchanged: %s', (text) => {
    expect(parsePointDescription(text)).toEqual({
      segments: [{ kind: 'text', text }],
      entryCount: 0,
    })
  })

  it('supports a single explicit or line-led definition', () => {
    expect(entries('Mode. Enumeration: <0> Reserved')).toHaveLength(1)
    expect(entries('Mode.\n <0> Reserved')).toHaveLength(1)
  })
})
