// Regression test for a blank-Curves-tab crash: `curve.curve_type.value` of
// 99 has no matching entry in the backend's curve-type enum (0-16), and
// `curveValueToEntry` used to throw for any unmapped code, unmounting the
// whole React root with no error boundary.
//
// Fix: `curveValueToEntry` returns `null` instead of throwing; `CurvesTab`
// renders an explicit "Invalid curve type" message (the real code, from
// `curveTypeRawValue`) in place of the fields that need a resolved entry.

import { describe, it, expect, vi } from 'vitest'
import { render, screen } from '@testing-library/react'
import { Theme } from '@radix-ui/themes'
import CurvesTab from './CurvesTab'
import { EnumDataContext } from '@/contexts/useEnumData'
import type {
  EnumsResponse,
  PicsProfile,
  ValidationError,
} from '@/api/generated'
import invalidGeneralJson from '../../../tests/fixtures/profiles/invalid_general.json'
import invalidAiJson from '../../../tests/fixtures/profiles/invalid_ai.json'
import fullProfileJson from '../../../../data/profiles/full.json'

// Real curve-type enum (codes 0-16). Deliberately does NOT include 99, the
// invalid code both fixtures below carry, so the test exercises the real
// "no match" path.
const CURVE_TYPES_ENUM: EnumsResponse['curve_types'] = [
  { variant: 'NotDefined', display_name: 'Not Defined', std_number: 0 },
  {
    variant: 'NotApplicableUnknown',
    display_name: 'Not Applicable / Unknown',
    std_number: 1,
  },
  { variant: 'VoltVar', display_name: 'Volt-VAR', std_number: 2 },
  { variant: 'FrequencyWatt', display_name: 'Frequency-Watt', std_number: 3 },
  { variant: 'WattVar', display_name: 'Watt-VAR', std_number: 4 },
  { variant: 'VoltageWatt', display_name: 'Voltage-Watt', std_number: 5 },
  {
    variant: 'RemainConnected',
    display_name: 'Remain Connected',
    std_number: 6,
  },
  {
    variant: 'TemperatureMode',
    display_name: 'Temperature Mode',
    std_number: 7,
  },
  {
    variant: 'PricingSignalMode',
    display_name: 'Pricing Signal Mode',
    std_number: 8,
  },
  { variant: 'HvrtMustTrip', display_name: 'HVRT Must Trip', std_number: 9 },
  {
    variant: 'HvrtMomentaryCessation',
    display_name: 'HVRT Momentary Cessation',
    std_number: 10,
  },
  { variant: 'LvrtMustTrip', display_name: 'LVRT Must Trip', std_number: 11 },
  {
    variant: 'LvrtMomentaryCessation',
    display_name: 'LVRT Momentary Cessation',
    std_number: 12,
  },
  { variant: 'HfrtMustTrip', display_name: 'HFRT Must Trip', std_number: 13 },
  {
    variant: 'HfrtMomentaryCessation',
    display_name: 'HFRT Momentary Cessation',
    std_number: 14,
  },
  { variant: 'LfrtMustTrip', display_name: 'LFRT Must Trip', std_number: 15 },
  {
    variant: 'LfrtMomentaryCessation',
    display_name: 'LFRT Momentary Cessation',
    std_number: 16,
  },
]

const ENUMS_RESPONSE: EnumsResponse = {
  action_types: [],
  curve_types: CURVE_TYPES_ENUM,
  curve_x_units: [],
  curve_y_units: [],
  mode_types: [],
  schedule_interval_units: [],
}

function renderCurvesTab(
  profileData: PicsProfile,
  errors: ValidationError[] = [],
) {
  return render(
    <Theme>
      <EnumDataContext.Provider
        value={{ enumsResponse: ENUMS_RESPONSE, isLoading: false, error: null }}
      >
        <CurvesTab
          profileData={profileData}
          setProfileData={() => {}}
          errors={errors}
        />
      </EnumDataContext.Provider>
    </Theme>,
  )
}

describe('CurvesTab with a structurally invalid curve_type (#492)', () => {
  it('renders invalid_general.json (curve 0 has curve_type 99) without throwing, and shows the invalid-type error', () => {
    const profile = invalidGeneralJson as unknown as PicsProfile

    expect(() => renderCurvesTab(profile)).not.toThrow()

    // The real offending code is shown, not swallowed or defaulted.
    expect(screen.getByText('Invalid curve type (99)')).toBeInTheDocument()
    expect(screen.getByText(/Invalid curve type code 99:/)).toBeInTheDocument()
  })

  it('renders invalid_ai.json (curve 1 has curve_type 99) without throwing; selecting curve 1 shows the invalid-type error', async () => {
    const profile = invalidAiJson as unknown as PicsProfile

    expect(() => renderCurvesTab(profile)).not.toThrow()

    // Curve 0 renders cleanly by default; the bad curve is index 1.
    expect(
      screen.getByRole('heading', { name: 'HVRT Must Trip' }),
    ).toBeInTheDocument()

    const select = screen.getByLabelText('Select Curve')
    ;(select as HTMLSelectElement).value = '1'
    select.dispatchEvent(new Event('change', { bubbles: true }))

    expect(
      await screen.findByText('Invalid curve type (99)'),
    ).toBeInTheDocument()
  })

  it('still renders full.json, the known-good reference fixture, with no invalid-curve error anywhere', () => {
    const profile = fullProfileJson as unknown as PicsProfile

    expect(() => renderCurvesTab(profile)).not.toThrow()
    expect(screen.queryByText(/Invalid curve type/)).not.toBeInTheDocument()
    // The label also appears as an <option> in the Type select, so scope
    // to the heading specifically.
    expect(
      screen.getByRole('heading', { name: 'HVRT Must Trip' }),
    ).toBeInTheDocument()
  })

  // Copilot review finding on #526: the "Unrecognized code" placeholder
  // carries value "", and it used to be reachable via the Type select's
  // onChange, feeding '' into the typed CurveType handler and writing a
  // bogus curve_type. Selecting it (the already-selected placeholder) must
  // be a no-op: no write happens at all.
  it('selecting the "Unrecognized code" placeholder is a no-op: setProfileData is never called with it', () => {
    const profile = invalidGeneralJson as unknown as PicsProfile
    const setProfileData = vi.fn()

    render(
      <Theme>
        <EnumDataContext.Provider
          value={{
            enumsResponse: ENUMS_RESPONSE,
            isLoading: false,
            error: null,
          }}
        >
          <CurvesTab
            profileData={profile}
            setProfileData={setProfileData}
            errors={[]}
          />
        </EnumDataContext.Provider>
      </Theme>,
    )

    const typeSelect = screen.getByLabelText('Type') as HTMLSelectElement
    expect(typeSelect.value).toBe('')

    typeSelect.value = ''
    typeSelect.dispatchEvent(new Event('change', { bubbles: true }))

    expect(setProfileData).not.toHaveBeenCalled()
  })
})

// Bucketing (`validationBucketing.test.ts`) already proves the routing
// rule; these assert the routed errors actually render inside this tab's
// own panel, real message text and all.
describe('CurvesTab renders its own routed validation errors (#492, Craig 2026-08-18)', () => {
  // Real curve-structural errors from `invalid_ai.json` (see
  // App.test.tsx's `invalidAiErrors()` for the full capture): the four
  // that route to Curves.
  const CURVE_ERRORS_FROM_INVALID_AI: ValidationError[] = [
    {
      point:
        "<Curve of type 'Unknown' (4 points, x_units: TimeMs, y_units: VoltsPctVRef)>: curve type (AI329)",
      message:
        'Invalid curve type 99, expected a whole number between 0 and 16',
    },
    {
      point:
        "<Curve of type 'Unknown' (4 points, x_units: TimeMs, y_units: VoltsPctVRef)>: x_units (AI331)",
      message:
        'Curve type Unknown is not compatible with x_units TimeMs. Compatible units: [NotDefined]',
    },
    {
      point:
        "<Curve of type 'Unknown' (4 points, x_units: TimeMs, y_units: VoltsPctVRef)>: y_units (AI332)",
      message:
        'Curve type Unknown is not compatible with y_units VoltsPctVRef. Compatible units: [NotApplicable]',
    },
    {
      point:
        "<Curve of type 'HFRTMustTrip' (500 points, x_units: TimeMs, y_units: FrequencyPctNominal)>: number of points (AI330)",
      message:
        'Number of points must be a whole number between 0 and 100, got 500',
    },
  ]

  it('renders all four routed curve errors, with real message text, when the tab is given the Curves bucket', () => {
    const profile = invalidAiJson as unknown as PicsProfile
    renderCurvesTab(profile, CURVE_ERRORS_FROM_INVALID_AI)

    expect(
      screen.getByText(
        /Invalid curve type 99, expected a whole number between 0 and 16/,
      ),
    ).toBeInTheDocument()
    expect(
      screen.getByText(/not compatible with x_units TimeMs/),
    ).toBeInTheDocument()
    expect(
      screen.getByText(/not compatible with y_units VoltsPctVRef/),
    ).toBeInTheDocument()
    expect(
      screen.getByText(/Number of points must be a whole number/),
    ).toBeInTheDocument()

    // Tab badge count: driven off the same `errors.length` App.tsx passes
    // through `tabErrorCounts`.
    expect(screen.getByText('4')).toBeInTheDocument()
  })

  it('renders nothing when given an empty errors bucket: no stray callout shell', () => {
    const profile = invalidAiJson as unknown as PicsProfile
    renderCurvesTab(profile, [])

    expect(screen.queryByText(/Invalid curve type 99/)).not.toBeInTheDocument()
  })
})
