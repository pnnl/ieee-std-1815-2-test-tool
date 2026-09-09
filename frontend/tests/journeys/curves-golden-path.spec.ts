/**
 * Golden-path E2E for the Curves tab.
 *
 * Profile: mandatory_1547 seed (10 x/y slots, all values null). We save it
 * as a working copy under CURVES_TEST_PROFILE so it doesn't collide with
 * the global playwright_test_profile fixture.
 *
 * Covers:
 *   1. Open the mandatory_1547 profile
 *   2. Add a curve via the "Add" button
 *   3. For each of four curve types (Volt-Var, Frequency-Watt, Pricing
 *      signal mode, HVRT Must Trip): switch the type select, set a small
 *      number of points, fill point values, and assert the rendered values
 *      match what was entered.
 *
 * Fixture-based: beforeAll creates the profile; afterAll deletes it.
 */

import { test, expect, request as playwrightRequest } from '@playwright/test'
import { CURVE_X_UNITS, CURVE_Y_UNITS } from '../../src/utils/curveUtils'
import { PW_BASE_URL } from '../shared_config'
import type { CurveType } from '../../src/api/generated'

// Fixed name — stable across beforeAll, individual tests, and all Playwright
// workers. A dynamic Date.now() suffix breaks when fullyParallel splits each
// test into a separate worker process that imports the module independently
// and gets a different timestamp, causing beforeAll and the test body to
// disagree about which profile was created.
const CURVES_TEST_PROFILE = 'e2e_curves_golden_path'

// Numeric codes verified against CURVE_X_UNITS / CURVE_Y_UNITS in curveUtils.ts:
//   Volt-Var:       X=29 (Voltage V), Y=2  (% VarMax)
//   Frequency-Watt: X=33 (Frequency Hz), Y=5  (% Wmax)
//   Pricing:        X=100 (Price hundredths), Y=5  (% Wmax)
//   HVRT Must Trip: X=29 (Voltage V), Y=29 (Voltage V)
//     NOTE: "s" (seconds) is absent from CURVE_Y_UNITS. HVRT ride-through
//     time ideally uses seconds, but that code does not exist in the SoT.
//     Voltage (V) = 29 is used as the nearest available Y unit.
//
// The `satisfies` constraint enforces that each value is a numeric string
// (because CURVE_X_UNITS / CURVE_Y_UNITS are typed Record<number, string>,
// keyof widens to number); key-existence drift in curveUtils.ts is caught
// at runtime when selectOption() throws if the option is absent from the DOM.
const AXIS_UNITS = {
  VOLT_VAR: { x: '29', y: '2' },
  FREQ_WATT: { x: '33', y: '5' },
  PRICE: { x: '100', y: '5' },
  HVRT_MUST_TRIP: { x: '29', y: '29' },
} as const satisfies Record<
  string,
  { x: `${keyof typeof CURVE_X_UNITS}`; y: `${keyof typeof CURVE_Y_UNITS}` }
>

/**
 * Locate the <select> that is preceded by a <Label> whose text exactly
 * matches `labelText` within the Curve Editor panel.
 *
 * NativeSelect renders:
 *   <div class="flex flex-col gap-1">
 *     <Label>labelText</Label>
 *     <select>…</select>
 *   </div>
 *
 * The Label element has no `for`/`htmlFor`, so getByLabel() won't work.
 * We match on the label text, step to its parent div, then find the select.
 */
function curveSelect(page: import('@playwright/test').Page, labelText: string) {
  return page
    .getByText(labelText, { exact: true })
    .locator('xpath=..')
    .locator('select')
}

function pointInput(
  page: import('@playwright/test').Page,
  rowIndex: number,
  axis: 'x' | 'y',
) {
  // Each row has: index-cell | x-input-cell | y-input-cell
  const colIndex = axis === 'x' ? 1 : 2 // 0-based column among data cells
  return page
    .getByRole('row')
    .nth(rowIndex + 1) // +1 to skip the header row
    .getByRole('cell')
    .nth(colIndex)
    .locator('input[type="number"]')
}

// Other SVGs on the page are small icons; these dimensions target the chart precisely.
const CHART_SVG_LOCATOR = 'svg[width="500"][height="400"]'

/**
 * Return the "Number of Points" NumberInput (type="number") in the Curve Editor.
 *
 * NativeNumberInput renders:
 *   <div class="flex flex-col gap-1">
 *     <Label>Number of Points</Label>
 *     <input type="number" … />
 *   </div>
 *
 * The Label has no `for`/`htmlFor`, so getByLabel() won't work.
 * We match on the label text, step to its parent div, then find the input.
 */
function nopInput(page: import('@playwright/test').Page) {
  return page
    .getByText('Number of Points', { exact: true })
    .locator('xpath=..')
    .locator('input[type="number"]')
}

async function fillAndAssertPoints(
  page: import('@playwright/test').Page,
  ptData: ReadonlyArray<{ x: number; y: number }>,
): Promise<void> {
  for (let i = 0; i < ptData.length; i++) {
    await pointInput(page, i, 'x').fill(String(ptData[i].x))
    await pointInput(page, i, 'x').blur()
    await pointInput(page, i, 'y').fill(String(ptData[i].y))
    await pointInput(page, i, 'y').blur()
  }
  for (let i = 0; i < ptData.length; i++) {
    await expect(pointInput(page, i, 'x')).toHaveValue(String(ptData[i].x))
    await expect(pointInput(page, i, 'y')).toHaveValue(String(ptData[i].y))
  }
}

async function selectTypeAndAssertHeading(
  page: import('@playwright/test').Page,
  code: CurveType,
  expectedHeading: string,
): Promise<void> {
  await curveSelect(page, 'Type').selectOption(String(code))
  await expect(
    page.getByRole('heading', { name: expectedHeading }),
  ).toBeVisible()
}

async function setAndAssertAxisUnits(
  page: import('@playwright/test').Page,
  units: (typeof AXIS_UNITS)[keyof typeof AXIS_UNITS],
): Promise<void> {
  await curveSelect(page, 'X-Axis Units').selectOption(units.x)
  await expect(curveSelect(page, 'X-Axis Units')).toHaveValue(units.x)
  await curveSelect(page, 'Y-Axis Units').selectOption(units.y)
  await expect(curveSelect(page, 'Y-Axis Units')).toHaveValue(units.y)
}

async function expectChartVisible(
  page: import('@playwright/test').Page,
): Promise<void> {
  await expect(page.locator(CHART_SVG_LOCATOR)).toBeVisible()
}

// Force serial execution so all tests share the single-worker lifecycle.
// Without this, fullyParallel:true (local dev) can split workers and let one
// worker's afterAll delete the working profile while another is still using it.
test.describe.configure({ mode: 'serial' })

test.beforeAll(async () => {
  const apiContext = await playwrightRequest.newContext({
    baseURL: PW_BASE_URL,
  })
  try {
    const seedResp = await apiContext.get('/api/profiles/mandatory_1547')
    if (!seedResp.ok()) {
      throw new Error(
        `Could not fetch mandatory_1547 seed: HTTP ${seedResp.status()}`,
      )
    }
    const seedData = await seedResp.json()

    const saveResp = await apiContext.post('/api/profiles', {
      data: { name: CURVES_TEST_PROFILE, profile: seedData },
    })
    if (!saveResp.ok()) {
      throw new Error(
        `Could not create curves test profile: HTTP ${saveResp.status()}`,
      )
    }
  } finally {
    await apiContext.dispose()
  }
})

test.afterAll(async () => {
  const apiContext = await playwrightRequest.newContext({
    baseURL: PW_BASE_URL,
  })
  try {
    const resp = await apiContext.delete(`/api/profiles/${CURVES_TEST_PROFILE}`)
    if (!resp.ok() && resp.status() !== 404) {
      throw new Error(
        `Failed to delete curves test profile '${CURVES_TEST_PROFILE}': HTTP ${resp.status()}`,
      )
    }
  } finally {
    await apiContext.dispose()
  }
})

async function loadTestProfile(page: import('@playwright/test').Page) {
  // Load the app and wait for the bootstrap effect to finish loading
  // playwright_test_profile. Using a bare .waitFor({ state: 'visible' })
  // resolves immediately because the <p> is rendered before any profile loads
  // (it shows "no profile loaded"). We need to wait for the specific bootstrap
  // profile name so the bootstrap async chain is fully settled before we open
  // the modal — otherwise setCurrentProfileName from the bootstrap effect can
  // overwrite setCurrentProfileName from handleSelectProfile and the header
  // ends up showing playwright_test_profile instead of e2e_curves_golden_path.
  await page.goto('/')
  await expect(
    page.locator('p', { hasText: /^Current Profile: / }),
  ).toContainText('playwright_test_profile', { timeout: 15000 })

  // Open the Load Profile modal rather than manipulating localStorage directly.
  // The bootstrap persistence effect would overwrite localStorage before the
  // second page.goto fires, causing a race.
  await page.getByRole('button', { name: 'Load Profile' }).click()

  const profileRow = page
    .getByRole('row')
    .filter({ hasText: CURVES_TEST_PROFILE })
  await profileRow.waitFor({ state: 'visible', timeout: 10000 })
  await profileRow.getByRole('button', { name: 'Load' }).click()

  // Wait for the dialog to dismiss so the next interactions aren't intercepted.
  await expect(page.getByRole('dialog', { name: /load profile/i })).toBeHidden({
    timeout: 3000,
  })

  // Confirm the profile is now shown in the header (the "Current Profile: …"
  // indicator). Use the <p> locator with partial text to avoid a strict-mode
  // collision with the transient "Profile loaded successfully" toast, which
  // also contains the profile name.
  await expect(
    page.locator('p', { hasText: /^Current Profile:/ }),
  ).toContainText(CURVES_TEST_PROFILE, { timeout: 15000 })
}

async function openCurvesTabAndAddCurve(page: import('@playwright/test').Page) {
  await page.getByRole('tab', { name: 'Curves' }).click()
  await page.getByRole('button', { name: 'Add' }).click()

  // Wait for at least one curve option to appear, then pick the last one.
  // Targeting the last curve is defensive: if the fixture ever has fewer curves
  // than expected, we still select a valid entry rather than silently going
  // out-of-bounds on a hardcoded index.
  const selectCurve = curveSelect(page, 'Select Curve')
  await expect(selectCurve.locator('option[value]')).not.toHaveCount(0, {
    timeout: 5000,
  })

  const options = selectCurve.locator('option[value]')
  const count = await options.count()
  expect(count).toBeGreaterThan(0)

  const lastValue = await options.nth(count - 1).getAttribute('value')
  await selectCurve.selectOption(lastValue!)
  await expect(selectCurve).toHaveValue(lastValue!)
}

test('curves: volt-var — set type, set 3 points, assert rendered values', async ({
  page,
}) => {
  await loadTestProfile(page)

  await openCurvesTabAndAddCurve(page)

  await selectTypeAndAssertHeading(page, 'VoltVar', 'Volt-VAR')
  await setAndAssertAxisUnits(page, AXIS_UNITS.VOLT_VAR)

  await nopInput(page).fill('3')
  await nopInput(page).blur()

  await expect(page.getByRole('row')).toHaveCount(4) // 1 header + 3 data

  const ptData = [
    { x: 900, y: 100 },
    { x: 950, y: 50 },
    { x: 1000, y: 0 },
  ]
  await fillAndAssertPoints(page, ptData)

  await expect(curveSelect(page, 'Type')).toHaveValue('VoltVar')

  await expectChartVisible(page)
})

test('curves: frequency-watt — set type, set 2 points, assert rendered values', async ({
  page,
}) => {
  await loadTestProfile(page)
  await openCurvesTabAndAddCurve(page)

  await selectTypeAndAssertHeading(page, 'FrequencyWatt', 'Frequency-Watt')
  await setAndAssertAxisUnits(page, AXIS_UNITS.FREQ_WATT)

  await nopInput(page).fill('2')
  await nopInput(page).blur()

  await expect(page.getByRole('row')).toHaveCount(3) // 1 header + 2 data

  const ptData = [
    { x: 5990, y: 100 },
    { x: 6010, y: 0 },
  ]
  await fillAndAssertPoints(page, ptData)

  await expect(curveSelect(page, 'Type')).toHaveValue('FrequencyWatt')

  await expectChartVisible(page)
})

test('curves: price — set type, set 2 points, assert rendered values', async ({
  page,
}) => {
  await loadTestProfile(page)
  await openCurvesTabAndAddCurve(page)

  await selectTypeAndAssertHeading(
    page,
    'PricingSignalMode',
    'Pricing Signal Mode',
  )
  await setAndAssertAxisUnits(page, AXIS_UNITS.PRICE)

  await nopInput(page).fill('2')
  await nopInput(page).blur()

  await expect(page.getByRole('row')).toHaveCount(3)

  const ptData = [
    { x: 0, y: 100 },
    { x: 500, y: 50 },
  ]
  await fillAndAssertPoints(page, ptData)

  await expect(curveSelect(page, 'Type')).toHaveValue('PricingSignalMode')

  await expectChartVisible(page)
})

test('curves: hvrt-must-trip — set type, set 2 points, assert rendered values', async ({
  page,
}) => {
  await loadTestProfile(page)
  await openCurvesTabAndAddCurve(page)

  await selectTypeAndAssertHeading(page, 'HvrtMustTrip', 'HVRT Must Trip')
  await setAndAssertAxisUnits(page, AXIS_UNITS.HVRT_MUST_TRIP)

  await nopInput(page).fill('2')
  await nopInput(page).blur()

  await expect(page.getByRole('row')).toHaveCount(3)

  const ptData = [
    { x: 1100, y: 10 },
    { x: 1200, y: 1 },
  ]
  await fillAndAssertPoints(page, ptData)

  await expect(curveSelect(page, 'Type')).toHaveValue('HvrtMustTrip')

  await expectChartVisible(page)
})
