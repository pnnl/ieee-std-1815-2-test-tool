import { test, expect, request as playwrightRequest } from '@playwright/test'
import { PW_BASE_URL } from './shared_config'

// Loads a fresh working copy of the canonical `full` seed, edits an AI
// point's value out of its engineering range through the real UI input,
// presses Validate, and asserts the panel populates with the resulting
// error against the real backend, not a mock.

const FIXTURE_PROFILE = `e2e_492_validate_action_${Date.now()}`

test.describe('Validate action refreshes the panel against the current edit (#492)', () => {
  test.beforeAll(async () => {
    const apiContext = await playwrightRequest.newContext({
      baseURL: PW_BASE_URL,
    })
    try {
      const seedResponse = await apiContext.get('/api/profiles/full')
      if (!seedResponse.ok()) {
        throw new Error(
          `Failed to fetch canonical full seed: ${seedResponse.status()}`,
        )
      }
      const seed = await seedResponse.json()

      const saveResponse = await apiContext.post('/api/profiles', {
        data: { name: FIXTURE_PROFILE, profile: seed },
      })
      if (!saveResponse.ok()) {
        throw new Error(
          `Failed to create fixture profile: ${saveResponse.status()}`,
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
      const resp = await apiContext.delete(`/api/profiles/${FIXTURE_PROFILE}`)
      if (!resp.ok() && resp.status() !== 404) {
        console.warn(`Cleanup of ${FIXTURE_PROFILE} returned ${resp.status()}`)
      }
    } finally {
      await apiContext.dispose()
    }
  })

  test('obligation 16: editing a value out of range then pressing Validate populates the panel', async ({
    page,
  }) => {
    await page.goto('/')
    await expect(page.getByText(/Current Profile:/i)).toBeVisible({
      timeout: 15000,
    })

    await page
      .getByRole('button', { name: /^load profile$/i })
      .first()
      .click()
    const loadDialog = page.getByRole('dialog')
    await expect(loadDialog).toBeVisible({ timeout: 5000 })

    const myProfilesSection = loadDialog.locator(
      'section[aria-label="My Profiles"]',
    )
    await expect(myProfilesSection).toBeVisible()
    const fixtureRow = myProfilesSection
      .getByRole('row')
      .filter({ hasText: FIXTURE_PROFILE })
    await expect(fixtureRow).toBeVisible({ timeout: 5000 })
    await fixtureRow.getByRole('button', { name: 'Load' }).click()
    await expect(loadDialog).toBeHidden({ timeout: 5000 })

    await expect(
      page.getByText(new RegExp(`Current Profile:\\s*${FIXTURE_PROFILE}\\b`)),
    ).toBeVisible({ timeout: 10000 })

    // GET-based loads clear the panel unconditionally: confirms the
    // starting state before Validate.
    await expect(page.getByText('Profile validation issues')).toHaveCount(0)
    await expect(page.locator('.rt-CalloutRoot')).toHaveCount(0)

    await page.getByRole('tab', { name: 'Analog Inputs' }).click()
    const scadaButton = page.getByRole('button', { name: /\bScada\b/i }).first()
    await expect(scadaButton).toBeVisible()
    await scadaButton.click()

    // EntitiesTab stays mounted-but-hidden ahead of PointsTab in DOM
    // order, so a bare `input[type="number"]` resolves `.first()` to the
    // hidden Entities input. Scope to `:visible` to land on the real one.
    const valueInput = page.locator('input[type="number"]:visible').first()
    await expect(valueInput).toBeVisible()

    // Identify the point from the DOM's index cell rather than assuming
    // AI.points[0] from the seed.
    const editedRow = valueInput.locator('xpath=ancestor::tr[1]')
    const indexCellText = (
      await editedRow.locator('td').first().innerText()
    ).trim()
    const indexMatch = indexCellText.match(/^AI(\d+)$/)
    if (!indexMatch) {
      throw new Error(
        `Unexpected point index cell text for the edited row: "${indexCellText}"`,
      )
    }
    const editedPointIndex = Number(indexMatch[1])

    // Look up that point's range from the saved fixture and pick a value
    // guaranteed to sit outside it regardless of sign.
    const apiContext = await playwrightRequest.newContext({
      baseURL: PW_BASE_URL,
    })
    let outOfRangeValue: number
    try {
      const getResponse = await apiContext.get(
        `/api/profiles/${FIXTURE_PROFILE}`,
      )
      if (!getResponse.ok()) {
        throw new Error(
          `Failed to re-fetch fixture profile: ${getResponse.status()}`,
        )
      }
      const profile = await getResponse.json()
      const point = profile.AI.points.find(
        (p: { point_index: number }) => p.point_index === editedPointIndex,
      )
      if (!point) {
        throw new Error(
          `Edited point AI${editedPointIndex} not found in the fixture profile.`,
        )
      }
      const engMax = point.maximum * point.multiplier + point.offset
      outOfRangeValue = Math.round(engMax + 1e12)
    } finally {
      await apiContext.dispose()
    }

    await valueInput.fill(String(outOfRangeValue))

    await page.getByRole('button', { name: 'Validate', exact: true }).click()

    await expect(
      page.getByText(`AI${editedPointIndex}:`, { exact: false }),
    ).toBeVisible({ timeout: 10000 })
    // Scoped to the callout panel: PointRow also renders this message in
    // an `sr-only` span on the errored row, so a page-wide getByText
    // trips Playwright's strict-mode check.
    await expect(
      page.locator('.rt-CalloutRoot').getByText(/Value must be between/i),
    ).toBeVisible()
    // Still routed to the owning tab, not General.
    await expect(page.getByText('Profile validation issues')).toHaveCount(0)
  })
})
