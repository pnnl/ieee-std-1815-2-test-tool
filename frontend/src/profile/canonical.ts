// Canonical profile helpers.
//
// The frontend now operates on `PicsProfile` (the canonical document shape
// generated from `backend/src/common/src/profile/pics_profile.rs`). This module
// centralises the read/write helpers components need so the canonical schema
// surface lives in one place rather than being scattered across tabs.
//
// Design notes:
//   - Pure functions: every mutator returns a new profile (immutability rule).
//   - No legacy fallbacks: callers must hand in a valid `PicsProfile` or null.
//   - Section offsets mirror the legacy editor's UI labels but are derived from
//     index ranges in the canonical KeySheet rather than a bag of magic
//     numbers, so they stay accurate when seeds shift.

import type {
  AiBattery,
  AiDer,
  AiInverter,
  AiMeter,
  AiPoint,
  AiSchedule,
  AoBattery,
  AoInverter,
  AoMeter,
  AoPoint,
  BiBattery,
  BiDer,
  BiInverter,
  BiMeter,
  BiPoint,
  BoPoint,
  PicsProfile,
} from '@/api/generated'

// --------------------------------------------------------------------------
// Section types — the four point tabs the editor renders.
// --------------------------------------------------------------------------

export type PointSection =
  'binary_outputs' | 'binary_inputs' | 'analog_outputs' | 'analog_inputs'

const POINT_SECTIONS: PointSection[] = [
  'binary_outputs',
  'binary_inputs',
  'analog_outputs',
  'analog_inputs',
]

export function isPointSection(value: string): value is PointSection {
  return (POINT_SECTIONS as string[]).includes(value)
}

// --------------------------------------------------------------------------
// Equipment groups — for entity counts and equipment record manipulation.
// --------------------------------------------------------------------------

export type EquipmentGroup = 'meters' | 'ders' | 'inverters' | 'batteries'

// --------------------------------------------------------------------------
// Section offset map — used by PointsTab to label collapsible sections.
// Matches the legacy labels so the Playwright "Scada" assertion still finds
// the section header. Values are the inclusive lower-bound of each band.
// --------------------------------------------------------------------------

export const SECTION_OFFSETS: Record<string, number> = {
  scada: 0,
  gap_1: 586,
  schedules_v1: 2000,
  status_v1: 2300,
  schedules_v2: 3000,
  status_v2: 3500,
  gap_2: 4000,
  historical_meters: 5000,
  historical_ders: 10000,
  historical_inverters: 15000,
  historical_batteries: 20000,
  gap_3: 30000,
  vendor: 50000,
}

// --------------------------------------------------------------------------
// Array growth/shrink helper — used by every "set count" mutator.
//
// Semantics (uniform across all call sites):
//   - target === current length: return input untouched.
//   - target < current length: return arr.slice(0, target).
//   - target > current length and arr is empty: return input untouched
//     (no template to clone from; callers seed at least one entry first).
//   - target > current length: structuredClone the last element repeatedly
//     until length === target. `opts.onClone` lets a caller post-process the
//     clone (e.g. zero out a `value` field on a fresh point).
// --------------------------------------------------------------------------

function cloneClamp<T>(
  arr: readonly T[],
  target: number,
  opts?: { onClone?: (item: T) => T },
): T[] {
  if (arr.length === target) return arr as T[]
  if (arr.length > target) return arr.slice(0, target)
  if (arr.length === 0) return arr as T[]
  const out = [...arr]
  const onClone = opts?.onClone
  while (out.length < target) {
    const cloned = structuredClone(out[out.length - 1])
    out.push(onClone ? onClone(cloned) : cloned)
  }
  return out
}

// --------------------------------------------------------------------------
// Point flattening — collect every point in a section to feed the points-by-
// offset UI. Sub-struct order mirrors the canonical Rust iter_points methods.
// --------------------------------------------------------------------------

const AI_METER_FIELDS: ReadonlyArray<keyof AiMeter> = [
  'type_of_connection_point',
  'der_input_output_included',
  'type_of_circuit_phases',
  'apparent_power_calc_method',
  'frequency',
  'active_power',
  'active_power_a',
  'active_power_b',
  'active_power_c',
  'reactive_power',
  'reactive_power_a',
  'reactive_power_b',
  'reactive_power_c',
  'power_factor',
  'apparent_power',
  'phase_a_volts',
  'phase_a_angle',
  'phase_b_volts',
  'phase_b_angle',
  'phase_c_volts',
  'phase_c_angle',
  'avg_line_to_line_voltage',
  'current_a',
  'current_b',
  'current_c',
  'active_power_high_threshold',
  'active_power_low_threshold',
  'reactive_power_high_threshold',
  'reactive_power_low_threshold',
  'power_factor_high_threshold',
  'power_factor_low_threshold',
  'phase_a_volts_high_threshold',
  'phase_a_volts_low_threshold',
  'phase_b_volts_high_threshold',
  'phase_b_volts_low_threshold',
  'phase_c_volts_high_threshold',
  'phase_c_volts_low_threshold',
]

const AI_DER_FIELDS: ReadonlyArray<keyof AiDer> = [
  'unit_type',
  'nameplate_energy_capacity',
  'normal_operating_performance_category',
  'abnormal_operating_performance_category',
  'max_apparent_generation_power',
  'max_apparent_charging_power',
  'operational_time',
  'connection_time',
  'available_active_generation_power',
  'available_active_charging_power',
  'available_reactive_injection_power',
  'available_reactive_absorption_power',
  'non_impacting_injection_vars',
  'non_impacting_absorption_vars',
  'link_to_meter',
]

const AI_INVERTER_FIELDS: ReadonlyArray<keyof AiInverter> = [
  'apparent_power_calc_method',
  'active_power_target',
  'reactive_power_target',
  'active_power',
  'reactive_power',
  'power_factor',
  'apparent_power',
  'dc_input_power',
  'dc_voltage',
  'dc_current',
  'avg_line_to_neutral_voltage',
  'voltage_phase_a_to_b',
  'voltage_phase_b_to_c',
  'voltage_phase_c_to_a',
  'ac_current',
  'current_phase_a',
  'current_phase_b',
  'current_phase_c',
  'internal_temperature',
  'heat_sink_temperature',
  'transformer_temperature',
  'active_power_high_threshold',
  'active_power_low_threshold',
  'reactive_power_high_threshold',
  'reactive_power_low_threshold',
  'frequency_high_threshold',
  'frequency_low_threshold',
  'dc_input_power_high_threshold',
  'dc_input_power_low_threshold',
  'dc_current_high_threshold',
  'dc_current_low_threshold',
  'dc_voltage_high_threshold',
  'dc_voltage_low_threshold',
  'link_to_der_unit',
]

const AI_BATTERY_FIELDS: ReadonlyArray<keyof AiBattery> = [
  'type_of_storage',
  'nameplate_actual_capacity',
  'effective_capacity',
  'minimum_reserve',
  'maximum_reserve',
  'battery_state',
  'actual_state_of_charge',
  'state_of_health',
  'external_voltage',
  'internal_voltage',
  'current',
  'power',
  'min_cell_voltage',
  'max_cell_voltage',
  'min_temperature',
  'max_temperature',
  'external_ambient_temperature',
  'internal_ambient_temperature',
  'charge_current_limit',
  'discharge_current_limit',
  'min_voltage_limit',
  'max_voltage_limit',
  'connected_string_count',
  'external_voltage_high_threshold',
  'external_voltage_low_threshold',
  'internal_voltage_high_threshold',
  'internal_voltage_low_threshold',
  'link_to_inverter',
]

const BI_METER_FIELDS: ReadonlyArray<keyof BiMeter> = [
  'active_power_too_high',
  'active_power_too_low',
  'reactive_power_too_high',
  'reactive_power_too_low',
  'power_factor_too_high',
  'power_factor_too_low',
  'phase_a_voltage_too_high',
  'phase_a_voltage_too_low',
  'phase_b_voltage_too_high',
  'phase_b_voltage_too_low',
  'phase_c_voltage_too_high',
  'phase_c_voltage_too_low',
  'communication_error',
]

const BI_DER_FIELDS: ReadonlyArray<keyof BiDer> = [
  'maintenance_operational_state',
  'has_p1_alarms',
  'has_p2_alarms',
  'has_p3_alarms',
]

const BI_INVERTER_FIELDS: ReadonlyArray<keyof BiInverter> = [
  'active_power_too_high',
  'active_power_too_low',
  'reactive_power_too_high',
  'reactive_power_too_low',
  'frequency_too_high',
  'frequency_too_low',
  'dc_input_power_too_high',
  'dc_input_power_too_low',
  'dc_current_too_high',
  'dc_current_too_low',
  'dc_voltage_too_high',
  'dc_voltage_too_low',
  'power_factor_excitation',
  'communication_error',
  'local_control_mode',
  'dc_contactor_closed',
  'ground_fault_alarm',
  'dc_over_voltage_alarm',
  'dc_under_voltage_alarm',
  'ac_disconnect_warning',
  'dc_disconnect_warning',
  'grid_disconnect_warning',
  'cabinet_open_warning',
  'manual_shutdown_warning',
  'over_temperature_alarm',
  'under_temperature_alarm',
  'over_frequency_alarm',
  'under_frequency_alarm',
  'ac_over_voltage_alarm',
  'ac_under_voltage_alarm',
  'blown_string_fuse_alarm',
  'memory_loss_alarm',
  'hardware_test_failure',
  'other_alarm',
  'other_warning',
]

const BI_BATTERY_FIELDS: ReadonlyArray<keyof BiBattery> = [
  'status_of_storage',
  'communication_error',
  'local_control_mode',
  'dc_contactor_closed',
  'is_charging',
  'is_discharging',
  'external_voltage_too_high',
  'external_voltage_too_low',
  'internal_voltage_too_high',
  'internal_voltage_too_low',
  'over_temperature_alarm',
  'under_temperature_alarm',
  'temperature_imbalance_alarm',
  'over_temperature_warning',
  'under_temperature_warning',
  'temperature_imbalance_warning',
  'over_charge_current_alarm',
  'over_discharge_current_alarm',
  'over_charge_current_warning',
  'over_discharge_current_warning',
  'voltage_imbalance_warning',
  'current_imbalance_warning',
  'over_voltage_alarm',
  'under_voltage_alarm',
  'over_voltage_warning',
  'under_voltage_warning',
  'over_soc_max_alarm',
  'under_soc_min_alarm',
  'over_soc_max_warning',
  'under_soc_min_warning',
  'contactor_failure',
  'fan_error',
  'ground_fault',
  'door_open_alarm',
  'configuration_error',
  'configuration_warning',
  'other_alarm',
  'other_warning',
  'fire_alarm',
  'fire_supervisory_warning',
  'fire_trouble_warning',
  'fire_power_fault_warning',
  'chiller_alarm',
  'chiller_warning',
  'air_handler_alarm',
  'air_handler_warning',
  'fluid_alarm',
  'fluid_warning',
  'gas_alarm',
  'gas_warning',
  'electrolyte_alarm',
  'electrolyte_warning',
  'electrical_alarm',
  'electrical_warning',
]

const AO_METER_FIELDS: ReadonlyArray<keyof AoMeter> = [
  'active_power_high_threshold',
  'active_power_low_threshold',
  'reactive_power_high_threshold',
  'reactive_power_low_threshold',
  'power_factor_high_threshold',
  'power_factor_low_threshold',
  'phase_a_volts_high_threshold',
  'phase_a_volts_low_threshold',
  'phase_b_volts_high_threshold',
  'phase_b_volts_low_threshold',
  'phase_c_volts_high_threshold',
  'phase_c_volts_low_threshold',
]

const AO_INVERTER_FIELDS: ReadonlyArray<keyof AoInverter> = [
  'active_power_high_threshold',
  'active_power_low_threshold',
  'reactive_power_high_threshold',
  'reactive_power_low_threshold',
  'frequency_high_threshold',
  'frequency_low_threshold',
  'dc_input_power_high_threshold',
  'dc_input_power_low_threshold',
  'dc_current_high_threshold',
  'dc_current_low_threshold',
  'dc_voltage_high_threshold',
  'dc_voltage_low_threshold',
]

const AO_BATTERY_FIELDS: ReadonlyArray<keyof AoBattery> = [
  'external_voltage_high_threshold',
  'external_voltage_low_threshold',
  'internal_voltage_high_threshold',
  'internal_voltage_low_threshold',
]

// --------------------------------------------------------------------------
// Flattened views — every point in a section, indexed by `point_index`.
// --------------------------------------------------------------------------

export interface FlatBoPoint extends BoPoint {
  kind: 'bo'
  locator: BoLocator
}
export interface FlatBiPoint extends BiPoint {
  kind: 'bi'
  locator: BiLocator
}
export interface FlatAoPoint extends AoPoint {
  kind: 'ao'
  locator: AoLocator
}
export interface FlatAiPoint extends AiPoint {
  kind: 'ai'
  locator: AiLocator
}

export type FlatPoint = FlatBoPoint | FlatBiPoint | FlatAoPoint | FlatAiPoint

// Locators — opaque addresses that updateFlatPoint uses to write back the
// edited point into the right canonical sub-struct.

export type BoLocator = { kind: 'bo'; group: 'points'; index: number }

export type BiLocator =
  | { kind: 'bi'; group: 'points'; index: number }
  | {
      kind: 'bi'
      group: 'meters' | 'ders' | 'inverters' | 'batteries'
      recordIndex: number
      field: string
    }

export type AoLocator =
  | { kind: 'ao'; group: 'points'; index: number }
  | {
      kind: 'ao'
      group: 'meters' | 'inverters' | 'batteries'
      recordIndex: number
      field: string
    }

export type AiLocator =
  | { kind: 'ai'; group: 'points'; index: number }
  | {
      kind: 'ai'
      group: 'meters' | 'ders' | 'inverters' | 'batteries'
      recordIndex: number
      field: string
    }
  | {
      kind: 'ai'
      group: 'curve_header'
      recordIndex: number
      field: 'curve_type' | 'number_of_points' | 'x_units' | 'y_units'
    }
  | {
      kind: 'ai'
      group: 'curve_x' | 'curve_y'
      recordIndex: number
      pointIndex: number
    }
  | {
      kind: 'ai'
      group: 'schedule_bc_header'
      recordIndex: number
      field:
        | 'identity'
        | 'priority'
        | 'schedule_type'
        | 'start_date'
        | 'start_time'
        | 'repeat_interval'
        | 'repeat_interval_units'
        | 'validation_status'
        | 'status'
        | 'number_of_points'
    }
  | {
      kind: 'ai'
      group: 'schedule_bc_offsets' | 'schedule_bc_values'
      recordIndex: number
      pointIndex: number
    }
  | {
      kind: 'ai'
      group: 'schedule_header'
      recordIndex: number
      field:
        | 'identity'
        | 'priority'
        | 'start_date'
        | 'start_time'
        | 'stop_date'
        | 'stop_time'
        | 'repeat_interval'
        | 'repeat_interval_units'
        | 'validation_state'
        | 'status'
        | 'number_of_points'
    }
  | {
      kind: 'ai'
      group:
        | 'schedule_offsets'
        | 'schedule_action_types'
        | 'schedule_action_indexes'
        | 'schedule_values'
      recordIndex: number
      pointIndex: number
    }

// --------------------------------------------------------------------------
// Section flatteners — produce one ordered list of points, with locators.
// --------------------------------------------------------------------------

export function flattenBinaryOutputs(profile: PicsProfile): FlatBoPoint[] {
  return profile.BO.points.map((p, i) => ({
    ...p,
    kind: 'bo' as const,
    locator: { kind: 'bo', group: 'points', index: i },
  }))
}

export function flattenBinaryInputs(profile: PicsProfile): FlatBiPoint[] {
  const out: FlatBiPoint[] = profile.BI.points.map((p, i) => ({
    ...p,
    kind: 'bi' as const,
    locator: { kind: 'bi', group: 'points', index: i },
  }))
  profile.BI.meters.forEach((rec, recordIndex) => {
    BI_METER_FIELDS.forEach((field) => {
      const pt = rec[field]
      out.push({
        ...pt,
        kind: 'bi',
        locator: { kind: 'bi', group: 'meters', recordIndex, field },
      })
    })
  })
  profile.BI.ders.forEach((rec, recordIndex) => {
    BI_DER_FIELDS.forEach((field) => {
      const pt = rec[field]
      out.push({
        ...pt,
        kind: 'bi',
        locator: { kind: 'bi', group: 'ders', recordIndex, field },
      })
    })
  })
  profile.BI.inverters.forEach((rec, recordIndex) => {
    BI_INVERTER_FIELDS.forEach((field) => {
      const pt = rec[field]
      out.push({
        ...pt,
        kind: 'bi',
        locator: { kind: 'bi', group: 'inverters', recordIndex, field },
      })
    })
  })
  profile.BI.batteries.forEach((rec, recordIndex) => {
    BI_BATTERY_FIELDS.forEach((field) => {
      const pt = rec[field]
      out.push({
        ...pt,
        kind: 'bi',
        locator: { kind: 'bi', group: 'batteries', recordIndex, field },
      })
    })
  })
  return out
}

export function flattenAnalogOutputs(profile: PicsProfile): FlatAoPoint[] {
  const out: FlatAoPoint[] = profile.AO.points.map((p, i) => ({
    ...p,
    kind: 'ao' as const,
    locator: { kind: 'ao', group: 'points', index: i },
  }))
  profile.AO.meters.forEach((rec, recordIndex) => {
    AO_METER_FIELDS.forEach((field) => {
      const pt = rec[field]
      out.push({
        ...pt,
        kind: 'ao',
        locator: { kind: 'ao', group: 'meters', recordIndex, field },
      })
    })
  })
  profile.AO.inverters.forEach((rec, recordIndex) => {
    AO_INVERTER_FIELDS.forEach((field) => {
      const pt = rec[field]
      out.push({
        ...pt,
        kind: 'ao',
        locator: { kind: 'ao', group: 'inverters', recordIndex, field },
      })
    })
  })
  profile.AO.batteries.forEach((rec, recordIndex) => {
    AO_BATTERY_FIELDS.forEach((field) => {
      const pt = rec[field]
      out.push({
        ...pt,
        kind: 'ao',
        locator: { kind: 'ao', group: 'batteries', recordIndex, field },
      })
    })
  })
  return out
}

export function flattenAnalogInputs(profile: PicsProfile): FlatAiPoint[] {
  const out: FlatAiPoint[] = profile.AI.points.map((p, i) => ({
    ...p,
    kind: 'ai' as const,
    locator: { kind: 'ai', group: 'points', index: i },
  }))
  profile.AI.meters.forEach((rec, recordIndex) => {
    AI_METER_FIELDS.forEach((field) => {
      out.push({
        ...rec[field],
        kind: 'ai',
        locator: { kind: 'ai', group: 'meters', recordIndex, field },
      })
    })
  })
  profile.AI.ders.forEach((rec, recordIndex) => {
    AI_DER_FIELDS.forEach((field) => {
      out.push({
        ...rec[field],
        kind: 'ai',
        locator: { kind: 'ai', group: 'ders', recordIndex, field },
      })
    })
  })
  profile.AI.inverters.forEach((rec, recordIndex) => {
    AI_INVERTER_FIELDS.forEach((field) => {
      out.push({
        ...rec[field],
        kind: 'ai',
        locator: { kind: 'ai', group: 'inverters', recordIndex, field },
      })
    })
  })
  profile.AI.batteries.forEach((rec, recordIndex) => {
    AI_BATTERY_FIELDS.forEach((field) => {
      out.push({
        ...rec[field],
        kind: 'ai',
        locator: { kind: 'ai', group: 'batteries', recordIndex, field },
      })
    })
  })
  // Curves and schedules — the headers + parallel arrays become individual
  // FlatAiPoints so the Points tab can display them under their offset bands.
  profile.AI.curves.forEach((curve, recordIndex) => {
    const headerFields: Array<
      'curve_type' | 'number_of_points' | 'x_units' | 'y_units'
    > = ['curve_type', 'number_of_points', 'x_units', 'y_units']
    headerFields.forEach((field) => {
      out.push({
        ...curve[field],
        kind: 'ai',
        locator: { kind: 'ai', group: 'curve_header', recordIndex, field },
      })
    })
    curve.x_values.forEach((pt, pointIndex) => {
      out.push({
        ...pt,
        kind: 'ai',
        locator: { kind: 'ai', group: 'curve_x', recordIndex, pointIndex },
      })
    })
    curve.y_values.forEach((pt, pointIndex) => {
      out.push({
        ...pt,
        kind: 'ai',
        locator: { kind: 'ai', group: 'curve_y', recordIndex, pointIndex },
      })
    })
  })
  profile.AI.schedules_bc.forEach((sched, recordIndex) => {
    const headerFields: Array<
      | 'identity'
      | 'priority'
      | 'schedule_type'
      | 'start_date'
      | 'start_time'
      | 'repeat_interval'
      | 'repeat_interval_units'
      | 'validation_status'
      | 'status'
      | 'number_of_points'
    > = [
      'identity',
      'priority',
      'schedule_type',
      'start_date',
      'start_time',
      'repeat_interval',
      'repeat_interval_units',
      'validation_status',
      'status',
      'number_of_points',
    ]
    headerFields.forEach((field) => {
      out.push({
        ...sched[field],
        kind: 'ai',
        locator: {
          kind: 'ai',
          group: 'schedule_bc_header',
          recordIndex,
          field,
        },
      })
    })
    sched.time_offsets.forEach((pt, pointIndex) => {
      out.push({
        ...pt,
        kind: 'ai',
        locator: {
          kind: 'ai',
          group: 'schedule_bc_offsets',
          recordIndex,
          pointIndex,
        },
      })
    })
    sched.values.forEach((pt, pointIndex) => {
      out.push({
        ...pt,
        kind: 'ai',
        locator: {
          kind: 'ai',
          group: 'schedule_bc_values',
          recordIndex,
          pointIndex,
        },
      })
    })
  })
  profile.AI.schedules.forEach((sched, recordIndex) => {
    const headerFields: Array<
      | 'identity'
      | 'priority'
      | 'start_date'
      | 'start_time'
      | 'stop_date'
      | 'stop_time'
      | 'repeat_interval'
      | 'repeat_interval_units'
      | 'validation_state'
      | 'status'
      | 'number_of_points'
    > = [
      'identity',
      'priority',
      'start_date',
      'start_time',
      'stop_date',
      'stop_time',
      'repeat_interval',
      'repeat_interval_units',
      'validation_state',
      'status',
      'number_of_points',
    ]
    headerFields.forEach((field) => {
      out.push({
        ...sched[field],
        kind: 'ai',
        locator: { kind: 'ai', group: 'schedule_header', recordIndex, field },
      })
    })
    sched.time_offsets.forEach((pt, pointIndex) => {
      out.push({
        ...pt,
        kind: 'ai',
        locator: {
          kind: 'ai',
          group: 'schedule_offsets',
          recordIndex,
          pointIndex,
        },
      })
    })
    sched.action_types.forEach((pt, pointIndex) => {
      out.push({
        ...pt,
        kind: 'ai',
        locator: {
          kind: 'ai',
          group: 'schedule_action_types',
          recordIndex,
          pointIndex,
        },
      })
    })
    sched.action_indexes.forEach((pt, pointIndex) => {
      out.push({
        ...pt,
        kind: 'ai',
        locator: {
          kind: 'ai',
          group: 'schedule_action_indexes',
          recordIndex,
          pointIndex,
        },
      })
    })
    sched.values.forEach((pt, pointIndex) => {
      out.push({
        ...pt,
        kind: 'ai',
        locator: {
          kind: 'ai',
          group: 'schedule_values',
          recordIndex,
          pointIndex,
        },
      })
    })
  })
  return out
}

export function flattenSection(
  profile: PicsProfile,
  section: PointSection,
): FlatPoint[] {
  switch (section) {
    case 'binary_outputs':
      return flattenBinaryOutputs(profile)
    case 'binary_inputs':
      return flattenBinaryInputs(profile)
    case 'analog_outputs':
      return flattenAnalogOutputs(profile)
    case 'analog_inputs':
      return flattenAnalogInputs(profile)
  }
}

// --------------------------------------------------------------------------
// Updating points by locator. Returns a new profile.
// --------------------------------------------------------------------------

type EditableField =
  | 'name'
  | 'mandatory_1815'
  | 'mandatory_1547'
  | 'value'
  | 'purpose'
  | 'units'
  | 'iec_61850_uid'

export function updatePoint(
  profile: PicsProfile,
  locator: BoLocator | BiLocator | AoLocator | AiLocator,
  field: EditableField,
  value: string | number | boolean | null,
): PicsProfile {
  const next = structuredClone(profile)
  const point = resolvePoint(next, locator)
  if (!point) {
    throw new Error(
      `Could not resolve point at locator ${JSON.stringify(locator)}`,
    )
  }
  // `value` only exists on AiPoint; the canonical schema has no `value` on
  // AO/BI/BO. Callers should not request `value` updates on
  // non-AI points. We guard in case anything slips through.
  if (field === 'value' && locator.kind !== 'ai') {
    throw new Error(
      `Cannot edit 'value' on ${locator.kind} points; only AI carries a value.`,
    )
  }
  // Type-erased write — the resolved point is one of the canonical point
  // structs which all share `name`, `mandatory_*`, `purpose`, etc.
  ;(point as Record<string, unknown>)[field] = value
  return next
}

function resolvePoint(
  profile: PicsProfile,
  locator: BoLocator | BiLocator | AoLocator | AiLocator,
): BoPoint | BiPoint | AoPoint | AiPoint | null {
  if (locator.kind === 'bo') return profile.BO.points[locator.index] ?? null
  if (locator.kind === 'bi') {
    if (locator.group === 'points')
      return profile.BI.points[locator.index] ?? null
    const rec = profile.BI[locator.group][locator.recordIndex]
    if (!rec) return null
    return (rec as unknown as Record<string, BiPoint>)[locator.field] ?? null
  }
  if (locator.kind === 'ao') {
    if (locator.group === 'points')
      return profile.AO.points[locator.index] ?? null
    const rec = profile.AO[locator.group][locator.recordIndex]
    if (!rec) return null
    return (rec as unknown as Record<string, AoPoint>)[locator.field] ?? null
  }
  // AI
  if (locator.group === 'points')
    return profile.AI.points[locator.index] ?? null
  if (
    locator.group === 'meters' ||
    locator.group === 'ders' ||
    locator.group === 'inverters' ||
    locator.group === 'batteries'
  ) {
    const rec = profile.AI[locator.group][locator.recordIndex]
    if (!rec) return null
    return (rec as unknown as Record<string, AiPoint>)[locator.field] ?? null
  }
  if (locator.group === 'curve_header') {
    const curve = profile.AI.curves[locator.recordIndex]
    if (!curve) return null
    return curve[locator.field] ?? null
  }
  if (locator.group === 'curve_x') {
    return (
      profile.AI.curves[locator.recordIndex]?.x_values[locator.pointIndex] ??
      null
    )
  }
  if (locator.group === 'curve_y') {
    return (
      profile.AI.curves[locator.recordIndex]?.y_values[locator.pointIndex] ??
      null
    )
  }
  if (locator.group === 'schedule_bc_header') {
    const sched = profile.AI.schedules_bc[locator.recordIndex]
    if (!sched) return null
    return sched[locator.field] ?? null
  }
  if (locator.group === 'schedule_bc_offsets') {
    return (
      profile.AI.schedules_bc[locator.recordIndex]?.time_offsets[
        locator.pointIndex
      ] ?? null
    )
  }
  if (locator.group === 'schedule_bc_values') {
    return (
      profile.AI.schedules_bc[locator.recordIndex]?.values[
        locator.pointIndex
      ] ?? null
    )
  }
  if (locator.group === 'schedule_header') {
    const sched = profile.AI.schedules[locator.recordIndex]
    if (!sched) return null
    return sched[locator.field] ?? null
  }
  if (locator.group === 'schedule_offsets') {
    return (
      profile.AI.schedules[locator.recordIndex]?.time_offsets[
        locator.pointIndex
      ] ?? null
    )
  }
  if (locator.group === 'schedule_action_types') {
    return (
      profile.AI.schedules[locator.recordIndex]?.action_types[
        locator.pointIndex
      ] ?? null
    )
  }
  if (locator.group === 'schedule_action_indexes') {
    return (
      profile.AI.schedules[locator.recordIndex]?.action_indexes[
        locator.pointIndex
      ] ?? null
    )
  }
  if (locator.group === 'schedule_values') {
    return (
      profile.AI.schedules[locator.recordIndex]?.values[locator.pointIndex] ??
      null
    )
  }
  return null
}

// --------------------------------------------------------------------------
// Equipment counts — read from the Key sheet (using BI count as the canonical
// source; in practice every group's bi/ai/ao/ctr counts agree). Writes
// expand/contract the per-group AI/BI/AO arrays in lockstep.
// --------------------------------------------------------------------------

export function getEquipmentCount(
  profile: PicsProfile,
  group: EquipmentGroup,
): number {
  // Map plural -> singular Key field.
  const keyField =
    group === 'meters'
      ? 'meter'
      : group === 'ders'
        ? 'der'
        : group === 'inverters'
          ? 'inverter'
          : 'battery'
  return profile.Key[keyField].bi.count
}

export function setEquipmentCount(
  profile: PicsProfile,
  group: EquipmentGroup,
  newCount: number,
): PicsProfile {
  const next = structuredClone(profile)
  const keyField =
    group === 'meters'
      ? 'meter'
      : group === 'ders'
        ? 'der'
        : group === 'inverters'
          ? 'inverter'
          : 'battery'

  // Sync the AI sub-arrays. AO is meter/inverter/battery only; BI/AI cover
  // all four. Expansion clones the last record (or no-ops if there are none,
  // leaving counts mismatched until a template is loaded).
  if (group === 'meters') {
    next.AI.meters = cloneClamp(next.AI.meters, newCount)
    next.BI.meters = cloneClamp(next.BI.meters, newCount)
    next.AO.meters = cloneClamp(next.AO.meters, newCount)
  } else if (group === 'ders') {
    next.AI.ders = cloneClamp(next.AI.ders, newCount)
    next.BI.ders = cloneClamp(next.BI.ders, newCount)
  } else if (group === 'inverters') {
    next.AI.inverters = cloneClamp(next.AI.inverters, newCount)
    next.BI.inverters = cloneClamp(next.BI.inverters, newCount)
    next.AO.inverters = cloneClamp(next.AO.inverters, newCount)
  } else {
    next.AI.batteries = cloneClamp(next.AI.batteries, newCount)
    next.BI.batteries = cloneClamp(next.BI.batteries, newCount)
    next.AO.batteries = cloneClamp(next.AO.batteries, newCount)
  }

  // Update Key counts. The KeySheet records counts per point-type (bo/bi/ao
  // /ai/ctr); we set them all to the new value because they always agree.
  next.Key[keyField].bo.count = newCount
  next.Key[keyField].bi.count = newCount
  next.Key[keyField].ao.count = newCount
  next.Key[keyField].ai.count = newCount
  next.Key[keyField].ctr.count = newCount
  return next
}

// --------------------------------------------------------------------------
// Curves — replace, add, remove, edit single x/y point.
// --------------------------------------------------------------------------

export function setCurveCount(
  profile: PicsProfile,
  target: number,
): PicsProfile {
  const next = structuredClone(profile)
  if (next.AI.curves.length === target) return next
  if (next.AI.curves.length > target) {
    next.AI.curves = next.AI.curves.slice(0, target)
    return next
  }
  if (next.AI.curves.length === 0) {
    // No template to clone from:
    // a profile must seed at least one curve before counts can grow. The
    // canonical seeds always ship with a curve.
    return next
  }
  const last = next.AI.curves[next.AI.curves.length - 1]
  while (next.AI.curves.length < target) {
    next.AI.curves.push(structuredClone(last))
  }
  return next
}

export function addCurve(profile: PicsProfile): PicsProfile {
  return setCurveCount(profile, profile.AI.curves.length + 1)
}

export function removeCurve(
  profile: PicsProfile,
  curveIndex: number,
): PicsProfile {
  if (curveIndex < 0 || curveIndex >= profile.AI.curves.length) return profile
  const next = structuredClone(profile)
  next.AI.curves.splice(curveIndex, 1)
  return next
}

export function updateCurveHeader(
  profile: PicsProfile,
  curveIndex: number,
  field: 'curve_type' | 'number_of_points' | 'x_units' | 'y_units',
  numericValue: number,
): PicsProfile {
  const curve = profile.AI.curves[curveIndex]
  if (!curve) return profile
  const next = structuredClone(profile)
  next.AI.curves[curveIndex][field].value = numericValue
  if (field === 'number_of_points') {
    // Clamp parallel arrays to the new length, padding with zero-valued
    // copies of the last point if growing.
    const zeroValue = (pt: AiPoint): AiPoint => {
      pt.value = 0
      return pt
    }
    const newCount = Math.max(0, numericValue | 0)
    next.AI.curves[curveIndex].x_values = cloneClamp(
      next.AI.curves[curveIndex].x_values,
      newCount,
      { onClone: zeroValue },
    )
    next.AI.curves[curveIndex].y_values = cloneClamp(
      next.AI.curves[curveIndex].y_values,
      newCount,
      { onClone: zeroValue },
    )
  }
  return next
}

export function updateCurvePointValue(
  profile: PicsProfile,
  curveIndex: number,
  axis: 'x' | 'y',
  pointIndex: number,
  numericValue: number,
): PicsProfile {
  const curve = profile.AI.curves[curveIndex]
  if (!curve) return profile
  const arr = axis === 'x' ? curve.x_values : curve.y_values
  if (pointIndex < 0 || pointIndex >= arr.length) return profile
  const next = structuredClone(profile)
  const target =
    axis === 'x'
      ? next.AI.curves[curveIndex].x_values
      : next.AI.curves[curveIndex].y_values
  target[pointIndex].value = numericValue
  return next
}

// --------------------------------------------------------------------------
// Schedules — work against canonical AiSchedule[] (1815.2 schedules). The
// editor never edited schedules_bc; that array stays as-loaded. All UI work
// targets `AI.schedules`.
// --------------------------------------------------------------------------

const MS_PER_DAY = 24 * 60 * 60 * 1000

function dateToEpochOffsets(date: Date | null): {
  days: number
  seconds: number
} {
  if (!date) return { days: 0, seconds: 0 }
  const totalMs = date.getTime()
  const days = Math.floor(totalMs / MS_PER_DAY)
  const remainder = totalMs % MS_PER_DAY
  const seconds = Math.floor(remainder / 1000)
  return { days, seconds }
}

function epochOffsetsToDate(days: number, seconds: number): Date | null {
  if (days === 0 && seconds === 0) return null
  return new Date(days * MS_PER_DAY + seconds * 1000)
}

export function setScheduleCount(
  profile: PicsProfile,
  target: number,
): PicsProfile {
  const next = structuredClone(profile)
  if (next.AI.schedules.length === target) return next
  if (next.AI.schedules.length > target) {
    next.AI.schedules = next.AI.schedules.slice(0, target)
    return next
  }
  if (next.AI.schedules.length === 0) return next
  const last = next.AI.schedules[next.AI.schedules.length - 1]
  while (next.AI.schedules.length < target) {
    const cloned = structuredClone(last)
    // Reset per-instance fields so callers don't see stale identity/priority.
    cloned.identity.value = next.AI.schedules.length + 1
    cloned.priority.value = next.AI.schedules.length + 1
    next.AI.schedules.push(cloned)
  }
  return next
}

export function updateSchedule(
  profile: PicsProfile,
  scheduleArrayIndex: number,
  patch: Partial<{
    identity: number
    priority: number
    startDate: Date
    stopDate: Date
    repeatInterval: number
    repeatIntervalUnit: number
  }>,
): PicsProfile {
  const sched = profile.AI.schedules[scheduleArrayIndex]
  if (!sched) return profile
  const next = structuredClone(profile)
  const target = next.AI.schedules[scheduleArrayIndex]
  if (patch.identity !== undefined) target.identity.value = patch.identity
  if (patch.priority !== undefined) target.priority.value = patch.priority
  if (patch.startDate !== undefined) {
    const off = dateToEpochOffsets(patch.startDate)
    target.start_date.value = off.days
    target.start_time.value = off.seconds
  }
  if (patch.stopDate !== undefined) {
    const off = dateToEpochOffsets(patch.stopDate)
    target.stop_date.value = off.days
    target.stop_time.value = off.seconds
  }
  if (patch.repeatInterval !== undefined)
    target.repeat_interval.value = patch.repeatInterval
  if (patch.repeatIntervalUnit !== undefined)
    target.repeat_interval_units.value = patch.repeatIntervalUnit
  return next
}

// View model — pulls schedule fields out into the shape consumed by the
// SchedulingTab/ScheduleEditor UI without requiring those components to know
// how AiPoint values are encoded.
export interface ScheduleView {
  arrayIndex: number // position in AI.schedules
  index: number // identity-derived display index (we use arrayIndex)
  identity: number
  priority: number
  startDate: Date | null
  stopDate: Date | null
  repeatInterval: number
  repeatIntervalUnit: number
  numberOfPoints: number
  // Action points — flat parallel-arrays distilled into one struct list.
  points: Array<{
    timeOffset: number
    actionType: number
    actionIndex: number
    value: number
  }>
}

export function viewSchedules(profile: PicsProfile): ScheduleView[] {
  return profile.AI.schedules.map((s, arrayIndex) => ({
    arrayIndex,
    index: arrayIndex,
    identity: numberOf(s.identity.value),
    priority: numberOf(s.priority.value),
    startDate: epochOffsetsToDate(
      numberOf(s.start_date.value),
      numberOf(s.start_time.value),
    ),
    stopDate: epochOffsetsToDate(
      numberOf(s.stop_date.value),
      numberOf(s.stop_time.value),
    ),
    repeatInterval: numberOf(s.repeat_interval.value),
    repeatIntervalUnit: numberOf(s.repeat_interval_units.value),
    numberOfPoints: numberOf(s.number_of_points.value),
    points: zipSchedulePoints(s),
  }))
}

function zipSchedulePoints(s: AiSchedule): Array<{
  timeOffset: number
  actionType: number
  actionIndex: number
  value: number
}> {
  const len = Math.min(
    s.time_offsets.length,
    s.action_types.length,
    s.action_indexes.length,
    s.values.length,
    numberOf(s.number_of_points.value),
  )
  const out: Array<{
    timeOffset: number
    actionType: number
    actionIndex: number
    value: number
  }> = []
  for (let i = 0; i < len; i++) {
    out.push({
      timeOffset: numberOf(s.time_offsets[i].value),
      actionType: numberOf(s.action_types[i].value),
      actionIndex: numberOf(s.action_indexes[i].value),
      value: numberOf(s.values[i].value),
    })
  }
  return out
}

function numberOf(value: number | null | undefined): number {
  if (value === null || value === undefined) return 0
  return value
}

// Canonical AiSchedule `point_index` numbering scheme.
//
// An AiSchedule has 11 header AiPoints (identity through `number_of_points`)
// followed by four parallel arrays: `time_offsets`, `action_types`,
// `action_indexes`, `values`. Each slot in those arrays is assigned a
// DNP3 AI index that interleaves the four arrays in stride-4 blocks:
//
//   slot 0: time_offsets[0]=base+0, action_types[0]=base+1,
//           action_indexes[0]=base+2, values[0]=base+3
//   slot 1: time_offsets[1]=base+4, action_types[1]=base+5,
//           action_indexes[1]=base+6, values[1]=base+7
//   ...
//
// where `base = number_of_points.point_index + 1`. So slot k of array a has:
//   point_index = number_of_points.point_index + 1 + a + 4*k
// with a in {0=time_offsets, 1=action_types, 2=action_indexes, 3=values}.
//
// Note: this describes the index *numbering*, not the Rust iter order.
// `AiSchedule::iter_points` yields the 11 headers then chains the four arrays
// end-to-end (all time_offsets, then all action_types, etc.) — not interleaved.
//
// We need the stride-4 numbering because `point_index` is the DNP3 AI
// index used by the backend (see indexed_db.rs).
// Synthesising points with `point_index = 0` or cloning across parallel
// arrays (which have different bases) produces a semantically invalid
// profile even if the UI no longer crashes.

const SCHEDULE_PARALLEL_ARRAY_COUNT = 4

type ScheduleArrayOrdinal = 0 | 1 | 2 | 3

// Build a fresh AiPoint with the correct point_index for a synthesised
// schedule slot. All other metadata is zero/empty — the canonical schedule
// helpers immediately overwrite the `.value` field, and the rest of the
// AiPoint metadata is reconstructed by the backend from its key sheet on
// reload.
function makeScheduleSlotPoint(point_index: number): AiPoint {
  return {
    point_index,
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
  }
}

// Grow/shrink one of a schedule's parallel arrays to `target` length,
// deriving any new slot's `point_index` from the schedule's own header
// (`number_of_points.point_index`) — NOT from a sibling array, which would
// carry the wrong DNP3 index. Existing slots are preserved verbatim so
// caller-supplied `point_index` values (loaded from the canonical profile)
// are never rewritten.
function resizeSchedulePointArray(
  arr: AiPoint[],
  target: number,
  arrayOrdinal: ScheduleArrayOrdinal,
  numberOfPointsAiIndex: number,
): AiPoint[] {
  if (arr.length === target) return arr
  if (arr.length > target) return arr.slice(0, target)

  const base = numberOfPointsAiIndex + 1 + arrayOrdinal
  const out = [...arr]
  while (out.length < target) {
    const slot = out.length
    out.push(makeScheduleSlotPoint(base + slot * SCHEDULE_PARALLEL_ARRAY_COUNT))
  }
  return out
}

// Replace the action-point array on a schedule. Pads parallel arrays out to
// the new length, deriving each new slot's `point_index` from the schedule's
// canonical layout so the wire-level DNP3 mapping stays correct even when
// the schedule shipped with empty parallel arrays.
export function setSchedulePoints(
  profile: PicsProfile,
  scheduleArrayIndex: number,
  points: Array<{
    timeOffset: number
    actionType: number
    actionIndex: number
    value: number
  }>,
): PicsProfile {
  const sched = profile.AI.schedules[scheduleArrayIndex]
  if (!sched) return profile
  const next = structuredClone(profile)
  const target = next.AI.schedules[scheduleArrayIndex]

  const numberOfPointsAiIndex = target.number_of_points.point_index
  target.time_offsets = resizeSchedulePointArray(
    target.time_offsets,
    points.length,
    0,
    numberOfPointsAiIndex,
  )
  target.action_types = resizeSchedulePointArray(
    target.action_types,
    points.length,
    1,
    numberOfPointsAiIndex,
  )
  target.action_indexes = resizeSchedulePointArray(
    target.action_indexes,
    points.length,
    2,
    numberOfPointsAiIndex,
  )
  target.values = resizeSchedulePointArray(
    target.values,
    points.length,
    3,
    numberOfPointsAiIndex,
  )

  for (let i = 0; i < points.length; i++) {
    target.time_offsets[i].value = points[i].timeOffset
    target.action_types[i].value = points[i].actionType
    target.action_indexes[i].value = points[i].actionIndex
    target.values[i].value = points[i].value
  }
  target.number_of_points.value = points.length

  return next
}
