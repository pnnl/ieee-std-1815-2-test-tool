// Maintainers can mark expected tests as implemented or not by editing
// the `implemented` boolean. IDs must exactly match the `test` field.

import type { TestName } from '@/api/generated/types.gen'

// emitted by the backend in `conformance_test_result` events.
export enum ImplementationStatus {
  Implemented = 'Implemented',
  PartiallyImplemented = 'Partially Implemented',
}

export type ControlModeTest = {
  testId: TestName
  implemented: ImplementationStatus
  displayName: string
}

type ControlModeTestMetadata = {
  implemented: ImplementationStatus
  displayName: string
}

export const testNameToControlModeTest = {
  MON_001: {
    implemented: ImplementationStatus.Implemented,
    displayName: 'Monitoring',
  },
  ALARM_001: {
    implemented: ImplementationStatus.Implemented,
    displayName: 'Alarm Reporting',
  },
  CONN_001: {
    implemented: ImplementationStatus.PartiallyImplemented,
    displayName: 'Connect and Disconnect',
  },
  SERV_001: {
    implemented: ImplementationStatus.PartiallyImplemented,
    displayName: 'Cease to Energize and Return to Service',
  },
  OP_001: {
    implemented: ImplementationStatus.PartiallyImplemented,
    displayName: 'Operation States',
  },
  CURVE_001: {
    implemented: ImplementationStatus.PartiallyImplemented,
    displayName: 'Reference Indication',
  },
  CURVE_002: {
    implemented: ImplementationStatus.PartiallyImplemented,
    displayName: 'Curve Locking',
  },
  CURVE_003: {
    implemented: ImplementationStatus.PartiallyImplemented,
    displayName: 'Mode Type Mismatch',
  },
  VRT_001: {
    implemented: ImplementationStatus.PartiallyImplemented,
    displayName: 'Voltage Ride-Through',
  },
  FRT_001: {
    implemented: ImplementationStatus.PartiallyImplemented,
    displayName: 'Frequency Ride-Through',
  },
  FW_001: {
    implemented: ImplementationStatus.PartiallyImplemented,
    displayName: 'Frequency-Watt',
  },
  DRCS_001: {
    implemented: ImplementationStatus.PartiallyImplemented,
    displayName: 'Dynamic Reactive Current Support',
  },
  DVW_001: {
    implemented: ImplementationStatus.PartiallyImplemented,
    displayName: 'Dynamic Volt-Watt',
  },
  APL_001: {
    implemented: ImplementationStatus.PartiallyImplemented,
    displayName: 'Active Power Limit',
  },
  CHG_001: {
    implemented: ImplementationStatus.PartiallyImplemented,
    displayName: 'Charge/Discharge Storage',
  },
  CCM_001: {
    implemented: ImplementationStatus.PartiallyImplemented,
    displayName: 'Coordinates Charge/Discharge Management',
  },
  APR1_001: {
    implemented: ImplementationStatus.PartiallyImplemented,
    displayName: 'Active Power Response 1',
  },
  APR2_001: {
    implemented: ImplementationStatus.PartiallyImplemented,
    displayName: 'Active Power Response 2',
  },
  APR3_001: {
    implemented: ImplementationStatus.PartiallyImplemented,
    displayName: 'Active Power Response 3',
  },
  AGC_001: {
    implemented: ImplementationStatus.PartiallyImplemented,
    displayName: 'Automatic Generation Control',
  },
  APS_001: {
    implemented: ImplementationStatus.PartiallyImplemented,
    displayName: 'Active Power Smoothing',
  },
  VW_001: {
    implemented: ImplementationStatus.PartiallyImplemented,
    displayName: 'Volt-Watt',
  },
  FWC_001: {
    implemented: ImplementationStatus.PartiallyImplemented,
    displayName: 'Frequency-Watt',
  },
  CVAR_001: {
    implemented: ImplementationStatus.PartiallyImplemented,
    displayName: 'Constant VAR',
  },
  FPF_001: {
    implemented: ImplementationStatus.PartiallyImplemented,
    displayName: 'Fixed Power Factor',
  },
  VV_001: {
    implemented: ImplementationStatus.PartiallyImplemented,
    displayName: 'Volt-VAR Control',
  },
  WV_001: {
    implemented: ImplementationStatus.PartiallyImplemented,
    displayName: 'Watt-VAR Control',
  },
  PFC_001: {
    implemented: ImplementationStatus.PartiallyImplemented,
    displayName: 'Power Factor Correction',
  },
  PSIG_001: {
    implemented: ImplementationStatus.PartiallyImplemented,
    displayName: 'Pricing Signal Control',
  },
  SCHED_001: {
    implemented: ImplementationStatus.PartiallyImplemented,
    displayName: 'Backward-Compatible Scheduling',
  },
  SCHED_002: {
    implemented: ImplementationStatus.PartiallyImplemented,
    displayName: 'Scheduling',
  },
} as const satisfies Record<string, ControlModeTestMetadata>
