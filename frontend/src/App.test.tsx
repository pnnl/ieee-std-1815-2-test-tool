// Flow-level tests for the validation-panel wiring. The SDK is mocked at
// the module boundary.
//
// Every test starts from the same bootstrap: `getProfile('full')` resolves
// with a real seed profile so the app has something loaded before the
// scenario under test runs.

import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import {
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { BrowserRouter } from 'react-router-dom'
import { Theme } from '@radix-ui/themes'
import App from './App'
import type {
  PicsProfile,
  ProfileListItem,
  ValidationError,
} from '@/api/generated'
import fullProfileJson from '../../data/profiles/full.json'

const FULL_PROFILE = fullProfileJson as unknown as PicsProfile

// A trimmed clone of FULL_PROFILE for the save-then-validate tests: the
// full profile flattens to ~826 Scada-band rows (curves/schedules included),
// which times out `getAllByRole('cell')` in jsdom. Points 0-9 keep the same
// Scada-band grouping the assertions rely on; everything else is zeroed.
const SMALL_AI_PROFILE: PicsProfile = {
  ...FULL_PROFILE,
  AI: {
    ...FULL_PROFILE.AI,
    points: FULL_PROFILE.AI.points.slice(0, 10),
    meters: [],
    ders: [],
    inverters: [],
    batteries: [],
    curves: [],
    schedules: [],
    schedules_bc: [],
  },
}

// jsdom's Blob/File implementation here does not expose `.text()`, which
// the JSON import path relies on. Polyfill it via FileReader.
if (typeof File !== 'undefined' && typeof File.prototype.text !== 'function') {
  File.prototype.text = function (this: File) {
    return new Promise<string>((resolve, reject) => {
      const reader = new FileReader()
      reader.onload = () => resolve(String(reader.result))
      reader.onerror = () => reject(reader.error)
      reader.readAsText(this)
    })
  }
}

const getProfile = vi.fn()
const listProfiles = vi.fn()
const saveProfile = vi.fn()
const validateProfile = vi.fn()

vi.mock('@/api/generated', async () => {
  const actual =
    await vi.importActual<typeof import('@/api/generated')>('@/api/generated')
  return {
    ...actual,
    getProfile: (...args: unknown[]) => getProfile(...args),
    listProfiles: (...args: unknown[]) => listProfiles(...args),
    saveProfile: (...args: unknown[]) => saveProfile(...args),
    validateProfile: (...args: unknown[]) => validateProfile(...args),
  }
})

function threeGeneralErrors(): ValidationError[] {
  // Non-mappable point strings: all three land in General regardless of
  // which tab is active.
  return [
    { point: 'N/A', message: 'workbook structural fault one' },
    { point: 'Key', message: 'workbook structural fault two' },
    { point: '', message: 'workbook structural fault three' },
  ]
}

function renderApp() {
  return render(
    <Theme>
      <BrowserRouter>
        <App />
      </BrowserRouter>
    </Theme>,
  )
}

// The header renders "Current Profile: {name}" as two sibling text nodes
// inside one <p>, so getByText('full') can't match either node alone.
function currentProfileName(): string {
  const el = screen.getByText(/Current Profile:/)
  return el.textContent?.replace('Current Profile:', '').trim() ?? ''
}

async function waitForBootstrapLoad() {
  await waitFor(() => expect(currentProfileName()).toBe('full'))
}

beforeEach(() => {
  vi.spyOn(window, 'confirm').mockReturnValue(true)
  vi.spyOn(window, 'prompt').mockReturnValue('imported_profile')

  getProfile.mockImplementation(
    async ({ path }: { path: { name: string } }) => {
      if (path.name === 'full') {
        return { data: FULL_PROFILE, error: undefined }
      }
      return { data: undefined, error: 'not found' }
    },
  )
  listProfiles.mockResolvedValue({
    data: [
      {
        filename: 'full.json',
        name: 'full',
        source: 'seed',
      } satisfies ProfileListItem,
    ],
    error: undefined,
  })
  saveProfile.mockResolvedValue({ data: { message: 'ok' }, error: undefined })
  validateProfile.mockResolvedValue({ data: {}, error: undefined })
})

afterEach(() => {
  vi.restoreAllMocks()
  vi.unstubAllGlobals()
})

describe('Validate action', () => {
  it('a mocked 400 carrying three errors populates the panel with exactly those three', async () => {
    renderApp()
    await waitForBootstrapLoad()

    validateProfile.mockResolvedValueOnce({
      data: undefined,
      error: { errors: { errors: threeGeneralErrors() } },
    })

    await userEvent.click(screen.getByRole('button', { name: /validate/i }))

    for (const error of threeGeneralErrors()) {
      expect(
        await screen.findByText(new RegExp(error.message)),
      ).toBeInTheDocument()
    }
  })

  it('a mocked 200 clears a previously populated panel', async () => {
    renderApp()
    await waitForBootstrapLoad()

    validateProfile.mockResolvedValueOnce({
      data: undefined,
      error: { errors: { errors: threeGeneralErrors() } },
    })
    await userEvent.click(screen.getByRole('button', { name: /validate/i }))
    await screen.findByText(/workbook structural fault one/)

    validateProfile.mockResolvedValueOnce({ data: {}, error: undefined })
    await userEvent.click(screen.getByRole('button', { name: /validate/i }))

    await waitFor(() =>
      expect(
        screen.queryByText(/workbook structural fault one/),
      ).not.toBeInTheDocument(),
    )
  })

  it('a mocked transport failure does not clear the panel to a false-clean state', async () => {
    renderApp()
    await waitForBootstrapLoad()

    validateProfile.mockResolvedValueOnce({
      data: undefined,
      error: { errors: { errors: threeGeneralErrors() } },
    })
    await userEvent.click(screen.getByRole('button', { name: /validate/i }))
    await screen.findByText(/workbook structural fault one/)

    // A transport failure: the SDK's client surfaces this as a truthy
    // `error` that does not match the structured ValidationErrorsResponse
    // shape.
    validateProfile.mockResolvedValueOnce({
      data: undefined,
      error: new TypeError('network down'),
    })
    await userEvent.click(screen.getByRole('button', { name: /validate/i }))

    // Distinguishable from "valid": the three prior errors are still shown.
    expect(
      await screen.findByText(/workbook structural fault one/),
    ).toBeInTheDocument()
    expect(
      screen.getByText(/workbook structural fault two/),
    ).toBeInTheDocument()
    expect(
      screen.getByText(/workbook structural fault three/),
    ).toBeInTheDocument()
  })
})

// An empty `errors` array is a legitimate "valid" answer; a missing, null,
// or non-array `errors` is not an answer at all and must never collapse
// into "clean" or an untouched-looking panel. Every test below starts from
// a populated panel so "left untouched" and "cleared to false-clean" are
// distinguishable on screen.
describe('Validate action: malformed /validate envelope hardening', () => {
  async function populatePanel() {
    renderApp()
    await waitForBootstrapLoad()

    validateProfile.mockResolvedValueOnce({
      data: undefined,
      error: { errors: { errors: threeGeneralErrors() } },
    })
    await userEvent.click(screen.getByRole('button', { name: /validate/i }))
    await screen.findByText(/workbook structural fault one/)
  }

  // No `<Toaster />` is mounted here, so the panel is the observable
  // signal: three prior errors staying on screen is the proof this was
  // not cleared to a false-clean state.

  it('the outer "errors" key missing entirely: panel left untouched, never cleared to clean', async () => {
    await populatePanel()

    validateProfile.mockResolvedValueOnce({ data: undefined, error: {} })
    await userEvent.click(screen.getByRole('button', { name: /validate/i }))

    expect(
      await screen.findByText(/workbook structural fault one/),
    ).toBeInTheDocument()
    expect(
      screen.getByText(/workbook structural fault two/),
    ).toBeInTheDocument()
    expect(
      screen.getByText(/workbook structural fault three/),
    ).toBeInTheDocument()
  })

  it('the inner "errors" array key missing (envelope present, array absent): panel left untouched, never cleared to clean', async () => {
    await populatePanel()

    validateProfile.mockResolvedValueOnce({
      data: undefined,
      error: { errors: {} },
    })
    await userEvent.click(screen.getByRole('button', { name: /validate/i }))

    expect(
      await screen.findByText(/workbook structural fault one/),
    ).toBeInTheDocument()
  })

  it('errors.errors: null: panel left untouched, never cleared to clean', async () => {
    await populatePanel()

    validateProfile.mockResolvedValueOnce({
      data: undefined,
      error: { errors: { errors: null } },
    })
    await userEvent.click(screen.getByRole('button', { name: /validate/i }))

    expect(
      await screen.findByText(/workbook structural fault one/),
    ).toBeInTheDocument()
  })

  it('errors.errors present but not an array: panel left untouched, never cleared to clean', async () => {
    await populatePanel()

    validateProfile.mockResolvedValueOnce({
      data: undefined,
      error: { errors: { errors: 'not-an-array' } },
    })
    await userEvent.click(screen.getByRole('button', { name: /validate/i }))

    expect(
      await screen.findByText(/workbook structural fault one/),
    ).toBeInTheDocument()
  })

  it('a well-formed but empty errors array is accepted, not routed to unconfirmed: no panel, no failure toast', async () => {
    renderApp()
    await waitForBootstrapLoad()

    validateProfile.mockResolvedValueOnce({
      data: undefined,
      error: { errors: { errors: [] } },
    })
    await userEvent.click(screen.getByRole('button', { name: /validate/i }))

    await waitFor(() =>
      expect(
        screen.queryByText('Profile validation issues'),
      ).not.toBeInTheDocument(),
    )
    expect(
      screen.queryByText(/Validate request failed/),
    ).not.toBeInTheDocument()
  })

  it('a malformed element missing "message" is degraded, not dropped: a placeholder is visibly shown', async () => {
    renderApp()
    await waitForBootstrapLoad()

    validateProfile.mockResolvedValueOnce({
      data: undefined,
      error: {
        errors: {
          errors: [{ point: 'N/A' }, ...threeGeneralErrors()],
        },
      },
    })
    await userEvent.click(screen.getByRole('button', { name: /validate/i }))

    expect(await screen.findByText(/malformed error entry/)).toBeInTheDocument()
    for (const error of threeGeneralErrors()) {
      expect(screen.getByText(new RegExp(error.message))).toBeInTheDocument()
    }
  })

  it('a malformed element missing "point" is degraded, not dropped: routes to General like any other unmapped point', async () => {
    renderApp()
    await waitForBootstrapLoad()

    validateProfile.mockResolvedValueOnce({
      data: undefined,
      error: {
        errors: { errors: [{ message: 'orphaned error, no point field' }] },
      },
    })
    await userEvent.click(screen.getByRole('button', { name: /validate/i }))

    expect(
      await screen.findByText(/orphaned error, no point field/),
    ).toBeInTheDocument()
  })
})

describe('Tab error count badges (review finding 1)', () => {
  it('shows the count on the owning tab, no badge on tabs with zero errors, from the same bucketing result the panels use', async () => {
    renderApp()
    await waitForBootstrapLoad()

    validateProfile.mockResolvedValueOnce({
      data: undefined,
      error: {
        errors: {
          errors: [
            { point: 'AI3', message: 'out of range' },
            { point: 'AI9', message: 'out of range' },
          ],
        },
      },
    })
    await userEvent.click(screen.getByRole('button', { name: /validate/i }))

    // The routed message never appears on the "entities" tab (Analog
    // Inputs isn't mounted here), so the badge is the only on-screen
    // signal. Wait on the badge's text, not the message.
    const analogInputsTab = screen.getByRole('tab', { name: /Analog Inputs/ })
    await waitFor(() =>
      expect(analogInputsTab).toHaveTextContent('2 validation errors'),
    )
    expect(screen.queryByText(/out of range/)).not.toBeInTheDocument()

    // Tabs with zero errors from this set render no badge: BO/BI have no
    // backend validation, and Entities never owns a bucket.
    for (const label of ['Binary Outputs', 'Binary Inputs', 'Entities']) {
      const tab = screen.getByRole('tab', { name: new RegExp(`^${label}`) })
      expect(tab.querySelector('[data-slot="badge"]')).toBeNull()
    }
  })

  it('the General panel gets the same badge treatment for errors that route to no tab', async () => {
    renderApp()
    await waitForBootstrapLoad()

    validateProfile.mockResolvedValueOnce({
      data: undefined,
      error: { errors: { errors: threeGeneralErrors() } },
    })
    await userEvent.click(screen.getByRole('button', { name: /validate/i }))
    const title = await screen.findByText('Profile validation issues')

    expect(title.parentElement).toHaveTextContent('3 validation errors')
  })
})

describe('JSON import', () => {
  it('a failing JSON import populates the panel AND loads the profile', async () => {
    renderApp()
    await waitForBootstrapLoad()

    validateProfile.mockResolvedValueOnce({
      data: undefined,
      // General-routed (point "N/A"): visible on the default tab with no
      // navigation needed. Per-prefix routing is covered in
      // validationBucketing.test.ts.
      error: {
        errors: { errors: [{ point: 'N/A', message: 'value out of range' }] },
      },
    })

    const file = new File([JSON.stringify(FULL_PROFILE)], 'broken.json', {
      type: 'application/json',
    })
    const input = document.getElementById('file-input') as HTMLInputElement
    await userEvent.upload(input, file)

    // Loaded despite the failure: the header now shows the imported name.
    await waitFor(() => expect(currentProfileName()).toBe('imported_profile'))
    expect(screen.getByText(/value out of range/)).toBeInTheDocument()
  })

  it('Cancelling the save-name prompt on a failing JSON import still surfaces the real /validate errors, not silence', async () => {
    // The file does not exist server-side, so handleImportFile takes the
    // "prompt for a save name" branch. Cancelling that prompt must not
    // discard already-computed `importValidationErrors`, or nothing loads
    // and nothing shows: a silent failure indistinguishable from a hang.
    vi.spyOn(window, 'prompt').mockReturnValue(null)

    renderApp()
    await waitForBootstrapLoad()

    validateProfile.mockResolvedValueOnce({
      data: undefined,
      error: {
        errors: {
          errors: [
            { point: 'AI0', message: 'Value must be between 1 and 100' },
            { point: 'AI3', message: 'Multiplier cannot be zero' },
          ],
        },
      },
    })

    saveProfile.mockClear()

    const file = new File([JSON.stringify(FULL_PROFILE)], 'invalid_ai.json', {
      type: 'application/json',
    })
    const input = document.getElementById('file-input') as HTMLInputElement
    await userEvent.upload(input, file)

    // The popup shows the actual errors the backend returned, not a
    // generic "import failed" message that hides which errors occurred.
    expect(await screen.findByText('Import failed')).toBeInTheDocument()
    expect(
      screen.getByText(/Value must be between 1 and 100/),
    ).toBeInTheDocument()
    expect(screen.getByText(/Multiplier cannot be zero/)).toBeInTheDocument()

    // The previously loaded profile is unchanged: no save was attempted.
    expect(currentProfileName()).toBe('full')
    expect(saveProfile).not.toHaveBeenCalled()
  })
})

describe('Load Profile from server', () => {
  // Real errors from `POST /api/profiles/validate` against
  // invalid_ai.json: 6 plain `AI<digits>` errors route to Analog Inputs,
  // and 4 curve-structural entries (display-name `point`, not matching the
  // anchored pattern) route to Curves, not General.
  function invalidAiErrors(): ValidationError[] {
    return [
      {
        point: 'AI333',
        message:
          'Expected minimum 0 per curve scaling table, got TransmissionI32(5)',
      },
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
      { point: 'AI0', message: 'Value must be between 1 and 100, but is 999' },
      {
        point: 'AI2',
        message: 'Minimum value cannot be greater than maximum value',
      },
      { point: 'AI2', message: 'Value must be between 10 and 0, but is 0' },
      { point: 'AI3', message: 'Multiplier cannot be zero' },
      {
        point: 'AI330',
        message: 'Value must be between 0 and 100, but is 500',
      },
    ]
  }

  it('loading invalid_ai.json from the modal calls /validate and routes its errors: AI-prefixed to Analog Inputs, curve-structural to Curves (Craig, 2026-08-18)', async () => {
    renderApp()
    await waitForBootstrapLoad()

    // Exercises the load-then-validate wiring, not the real validator,
    // which is exercised directly in offsetBands.test.ts.
    getProfile.mockImplementation(
      async ({ path }: { path: { name: string } }) => {
        if (path.name === 'full' || path.name === 'invalid_ai') {
          return { data: FULL_PROFILE, error: undefined }
        }
        return { data: undefined, error: 'not found' }
      },
    )
    listProfiles.mockResolvedValue({
      data: [
        { filename: 'full.json', name: 'full', source: 'seed' },
        { filename: 'invalid_ai.json', name: 'invalid_ai', source: 'seed' },
      ],
      error: undefined,
    })
    validateProfile.mockResolvedValueOnce({
      data: undefined,
      error: { errors: { errors: invalidAiErrors() } },
    })

    await userEvent.click(screen.getByRole('button', { name: /load profile/i }))
    const row = await screen.findByRole('row', { name: /invalid_ai/i })
    await userEvent.click(within(row).getByRole('button', { name: /load/i }))

    await waitFor(() => expect(currentProfileName()).toBe('invalid_ai'))

    // General panel: no errors route here now that curve-structural
    // errors go to Curves. The callout renders nothing, not an empty shell.
    await waitFor(() =>
      expect(
        screen.queryByText('Profile validation issues'),
      ).not.toBeInTheDocument(),
    )

    // Curves tab badge: the four curve-structural entries, each carrying a
    // display-name `point` matching the curve prefix, not the point pattern.
    const curvesTab = screen.getByRole('tab', { name: /Curves/ })
    await waitFor(() =>
      expect(curvesTab).toHaveTextContent('4 validation errors'),
    )

    // Analog Inputs tab badge: the six plain AI<digits> errors.
    const analogInputsTab = screen.getByRole('tab', { name: /Analog Inputs/ })
    await waitFor(() =>
      expect(analogInputsTab).toHaveTextContent('6 validation errors'),
    )

    // 4 (Curves) + 6 (Analog Inputs) + 0 (General) = 10, the backend's real
    // count. No content assertion for Curves here: switching to it mounts
    // `EnumDataProvider`, which calls the real `getEnums()` and would hang;
    // the routed curve errors' rendered text is asserted in
    // CurvesTab.test.tsx instead.

    // Switch to the tab and assert the actual rendered field values, not
    // just that something is present, per data-invariants Rule 1.
    await userEvent.click(analogInputsTab)
    expect(
      screen.getByText(/Value must be between 1 and 100, but is 999/),
    ).toBeInTheDocument()
    expect(
      screen.getByText(/Minimum value cannot be greater than maximum value/),
    ).toBeInTheDocument()
    expect(screen.getByText(/Multiplier cannot be zero/)).toBeInTheDocument()
    expect(
      screen.getByText(/Value must be between 0 and 100, but is 500/),
    ).toBeInTheDocument()
    expect(screen.getAllByText(/^AI2:/)).toHaveLength(2)
  })

  it('an unconfirmed /validate response during a load reads accurately, never claiming an import', async () => {
    renderApp()
    await waitForBootstrapLoad()

    getProfile.mockImplementation(
      async ({ path }: { path: { name: string } }) => {
        if (path.name === 'full' || path.name === 'unconfirmed_load') {
          return { data: FULL_PROFILE, error: undefined }
        }
        return { data: undefined, error: 'not found' }
      },
    )
    listProfiles.mockResolvedValue({
      data: [
        { filename: 'full.json', name: 'full', source: 'seed' },
        {
          filename: 'unconfirmed_load.json',
          name: 'unconfirmed_load',
          source: 'seed',
        },
      ],
      error: undefined,
    })
    validateProfile.mockResolvedValueOnce({
      data: undefined,
      error: 'network error',
    })

    await userEvent.click(screen.getByRole('button', { name: /load profile/i }))
    const row = await screen.findByRole('row', { name: /unconfirmed_load/i })
    await userEvent.click(within(row).getByRole('button', { name: /load/i }))

    await waitFor(() => expect(currentProfileName()).toBe('unconfirmed_load'))

    const message = await screen.findByText(
      /Profile validation could not be completed/,
    )
    expect(message.textContent).toContain('the profile was not confirmed valid')
    expect(message.textContent).not.toMatch(/imported/i)
  })

  it('the explicit Validate action on an already-loaded profile does the same routing (regression guard, not the bug under test)', async () => {
    renderApp()
    await waitForBootstrapLoad()

    validateProfile.mockResolvedValueOnce({
      data: undefined,
      error: { errors: { errors: [{ point: 'AI0', message: 'boom' }] } },
    })
    await userEvent.click(screen.getByRole('button', { name: /validate/i }))

    const analogInputsTab = screen.getByRole('tab', { name: /Analog Inputs/ })
    await waitFor(() =>
      expect(analogInputsTab).toHaveTextContent('1 validation error'),
    )
  })
})

// Save is never gated on validity, same as the load path. Once the save
// lands, the exact bytes just persisted are re-validated and the panel is
// replaced wholesale (never merged) through the same three-outcome
// `interpretValidateError` contract load and button-validate already use.
describe('Save persists, then re-validates the saved profile and replaces the panelw', () => {
  // Swap the bootstrap fixture for the trimmed profile (see
  // SMALL_AI_PROFILE) so expanding the Scada group stays fast.
  beforeEach(() => {
    getProfile.mockImplementation(
      async ({ path }: { path: { name: string } }) => {
        if (path.name === 'full') {
          return { data: SMALL_AI_PROFILE, error: undefined }
        }
        return { data: undefined, error: 'not found' }
      },
    )
  })

  // Meters is the first entity spinbutton; changing it flips `isModified`
  // and enables Save without touching the AI0-AI4 points below.
  async function markProfileModified() {
    const meters = screen.getAllByRole('spinbutton')[0] as HTMLInputElement
    fireEvent.change(meters, {
      target: { value: String(Number(meters.value) + 1) },
    })
    await waitFor(() =>
      expect(screen.getByRole('button', { name: 'Save' })).toBeEnabled(),
    )
  }

  // Drives the "Save Profile As" modal: the branch a fresh, seed-sourced
  // load takes on Save.
  async function clickSaveViaModal(name: string) {
    await userEvent.click(screen.getByRole('button', { name: 'Save' }))
    const dialog = await screen.findByRole('dialog')
    const input = within(dialog).getByLabelText('Profile name')
    await userEvent.clear(input)
    await userEvent.type(input, name)
    await userEvent.click(within(dialog).getByRole('button', { name: 'Save' }))
  }

  // Direct DOM traversal, not accessible-name matching: an errored cell's
  // sr-only span would break an exact match. Raw `querySelectorAll`, not
  // `getAllByRole('cell')`, which was measured at 20+ seconds here.
  function rowFor(pointLabel: string): HTMLElement {
    const cell = Array.from(document.querySelectorAll('td')).find((c) =>
      c.textContent?.startsWith(pointLabel),
    )
    if (!cell) throw new Error(`no cell found starting with "${pointLabel}"`)
    const row = cell.closest('tr')
    if (!row) throw new Error(`cell for "${pointLabel}" has no row ancestor`)
    return row as HTMLElement
  }

  function isErrorRow(el: HTMLElement): boolean {
    return el.className.includes('bg-destructive/10')
  }

  // The per-tab error list only exists in the DOM once the Analog Inputs
  // tab has been visited: every other tab is lazy-mounted and the default
  // is Entities.
  async function switchToAnalogInputs() {
    await userEvent.click(screen.getByRole('tab', { name: /Analog Inputs/ }))
  }

  // AI0/AI2/AI3/AI4 all fall in the Scada band; opening that one group
  // surfaces every row these tests touch.
  async function expandScadaGroup() {
    await userEvent.click(await screen.findByRole('button', { name: /Scada/ }))
  }

  it('replaces the panel wholesale on save: a fixed point disappears, a survivor stays, a new one appears, in the tab badge, group badge, and row highlight alike', async () => {
    renderApp()
    await waitForBootstrapLoad()

    // Pre-save: AI0, AI2, AI3 are errored (3 errors, all Scada).
    validateProfile.mockResolvedValueOnce({
      data: undefined,
      error: {
        errors: {
          errors: [
            { point: 'AI0', message: 'pre-save fault on AI0' },
            { point: 'AI2', message: 'pre-save fault on AI2' },
            { point: 'AI3', message: 'pre-save fault on AI3' },
          ],
        },
      },
    })
    await userEvent.click(screen.getByRole('button', { name: /validate/i }))
    await switchToAnalogInputs()
    await screen.findByText('pre-save fault on AI0')
    await expandScadaGroup()
    const analogInputsTab = screen.getByRole('tab', { name: /Analog Inputs/ })
    const scadaGroup = screen.getByRole('button', { name: /Scada/ })
    await waitFor(() =>
      expect(analogInputsTab).toHaveTextContent('3 validation errors'),
    )
    expect(scadaGroup).toHaveTextContent('3 validation errors')
    expect(isErrorRow(rowFor('AI0'))).toBe(true)
    expect(isErrorRow(rowFor('AI2'))).toBe(true)
    expect(isErrorRow(rowFor('AI3'))).toBe(true)
    expect(isErrorRow(rowFor('AI4'))).toBe(false)

    // Queued before saving, since it fires inside the same click: AI0/AI3
    // are fixed, AI2 survives, AI4 is new. A merged (not replaced) panel
    // would read 4, not 2.
    validateProfile.mockResolvedValueOnce({
      data: undefined,
      error: {
        errors: {
          errors: [
            { point: 'AI2', message: 'post-save fault on AI2' },
            { point: 'AI4', message: 'post-save fault on AI4' },
          ],
        },
      },
    })

    // The save must go through even though the profile is invalid.
    await markProfileModified()
    await clickSaveViaModal('saved_profile')

    await waitFor(() =>
      expect(analogInputsTab).toHaveTextContent('2 validation errors'),
    )
    expect(saveProfile).toHaveBeenCalled()
    expect(currentProfileName()).toBe('saved_profile')
    // Plural query form: an errored row's sr-only span repeats the
    // message alongside the panel's `<li>`, so both legitimately match.
    expect(
      screen.getAllByText('post-save fault on AI2').length,
    ).toBeGreaterThan(0)
    expect(
      screen.getAllByText('post-save fault on AI4').length,
    ).toBeGreaterThan(0)
    expect(screen.queryAllByText('pre-save fault on AI0')).toHaveLength(0)
    expect(screen.queryAllByText('pre-save fault on AI3')).toHaveLength(0)
    expect(scadaGroup).toHaveTextContent('2 validation errors')
    expect(isErrorRow(rowFor('AI0'))).toBe(false)
    expect(isErrorRow(rowFor('AI2'))).toBe(true)
    expect(isErrorRow(rowFor('AI3'))).toBe(false)
    expect(isErrorRow(rowFor('AI4'))).toBe(true)
  })

  it('a clean post-save validate clears the panel entirely: no General panel, no tab badge, no group badge, no row highlight', async () => {
    renderApp()
    await waitForBootstrapLoad()

    // One AI-routed and one General-routed error: both surfaces must
    // clear after save.
    validateProfile.mockResolvedValueOnce({
      data: undefined,
      error: {
        errors: {
          errors: [
            { point: 'AI0', message: 'pre-save fault on AI0' },
            { point: '', message: 'pre-save general fault' },
          ],
        },
      },
    })
    await userEvent.click(screen.getByRole('button', { name: /validate/i }))
    // The General-bucket error is App-level and visible immediately; the
    // AI-routed one isn't, until Analog Inputs has been visited.
    await screen.findByText('pre-save general fault')
    await switchToAnalogInputs()
    await screen.findByText('pre-save fault on AI0')
    await expandScadaGroup()
    const analogInputsTab = screen.getByRole('tab', { name: /Analog Inputs/ })
    await waitFor(() =>
      expect(analogInputsTab).toHaveTextContent('1 validation error'),
    )
    expect(isErrorRow(rowFor('AI0'))).toBe(true)

    validateProfile.mockResolvedValueOnce({ data: {}, error: undefined })
    await markProfileModified()
    await clickSaveViaModal('clean_saved_profile')

    await waitFor(() => expect(isErrorRow(rowFor('AI0'))).toBe(false))
    expect(currentProfileName()).toBe('clean_saved_profile')
    expect(
      screen.queryByText('Profile validation issues'),
    ).not.toBeInTheDocument()
    expect(screen.queryByText('pre-save general fault')).not.toBeInTheDocument()
    expect(analogInputsTab).not.toHaveTextContent(/validation error/)
    expect(screen.getByRole('button', { name: /Scada/ })).not.toHaveTextContent(
      /validation error/,
    )
  })

  it('a failed post-save validate request does not empty the panel: the save still succeeds, and the previous errors stay on screen with an explicit could-not-validate signal', async () => {
    renderApp()
    await waitForBootstrapLoad()

    validateProfile.mockResolvedValueOnce({
      data: undefined,
      error: {
        errors: { errors: [{ point: 'AI0', message: 'still broken' }] },
      },
    })
    await userEvent.click(screen.getByRole('button', { name: /validate/i }))
    await switchToAnalogInputs()
    await screen.findByText('still broken')
    await expandScadaGroup()
    const analogInputsTab = screen.getByRole('tab', { name: /Analog Inputs/ })
    await waitFor(() =>
      expect(analogInputsTab).toHaveTextContent('1 validation error'),
    )
    expect(isErrorRow(rowFor('AI0'))).toBe(true)

    // Not shaped as ValidationErrorsResponse, so `interpretValidateError`
    // reads `unconfirmed`. The save is a separate call and must not be
    // coupled to whether the post-save validate succeeds.
    validateProfile.mockResolvedValueOnce({
      data: undefined,
      error: 'network error',
    })
    await markProfileModified()
    await clickSaveViaModal('save_survives_validate_failure')

    // The save went through despite the validate call that followed
    // it failing.
    await waitFor(() =>
      expect(currentProfileName()).toBe('save_survives_validate_failure'),
    )
    expect(saveProfile).toHaveBeenCalled()

    // The panel was not emptied to a false-clean state: the prior error is
    // still shown in every surface that displayed it before.
    expect(screen.getAllByText('still broken').length).toBeGreaterThan(0)
    expect(analogInputsTab).toHaveTextContent('1 validation error')
    expect(screen.getByRole('button', { name: /Scada/ })).toHaveTextContent(
      '1 validation error',
    )
    expect(isErrorRow(rowFor('AI0'))).toBe(true)
  })
})
