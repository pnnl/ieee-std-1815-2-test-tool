// Curve UI constants: these enumerations stay frontend-only because the
// canonical schema stores curve_type/x_units/y_units as raw integer codes on
// AiPoint.value. The labels for those codes are presentation concerns.

import { CurveTypeEntry } from '@/api/generated'

export const MAX_CURVES = 10
export const MAX_CURVE_POINTS = 100

export interface CurvePoint {
  x: number
  y: number
}

export const CURVE_X_UNITS: Record<number, string> = {
  0: 'Not defined',
  1: 'Not applicable',
  4: 'Time (ms)',
  23: 'Celsius',
  29: 'Voltage (V)',
  33: 'Frequency (Hz)',
  38: 'Watts (W)',
  100: 'Price (hundredths)',
  129: '% Voltage',
  133: '% Frequency',
  138: '% Watts',
  233: 'Freq Deviation',
}

export const CURVE_Y_UNITS: Record<number, string> = {
  0: 'Not defined',
  1: 'Not applicable',
  2: '% VarMax',
  3: '% VarAval',
  4: '% Wmax (Vars)',
  5: '% Wmax',
  6: '% Frozen Power',
  7: 'Power Factor',
  8: '% VRef',
  9: '% Nominal Freq',
  29: 'Voltage (V)',
  33: 'Frequency (Hz)',
  38: 'Watts (W)',
}

// UI view of a curve: integer-coded fields plus an {x, y}[] derived from the
// canonical x_values/y_values parallel arrays. Lives on the consumer side; the
// canonical schema is the single source of truth.
//
// `curveTypeEntry` is `null` when `curveTypeRawValue` doesn't resolve to a
// known entry; `curveTypeRawValue` stays the raw code either way so a caller
// can still show the offending value.
export interface Curve {
  curveTypeEntry: CurveTypeEntry | null
  curveTypeRawValue: number
  number_of_points: number
  x_units: keyof typeof CURVE_X_UNITS
  y_units: keyof typeof CURVE_Y_UNITS
  points: CurvePoint[]
}
