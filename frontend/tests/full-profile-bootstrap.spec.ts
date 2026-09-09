import { test, expect, request as playwrightRequest } from '@playwright/test'

// Fresh browser context: no storage state, empty localStorage.
// This exercises the bootstrap default-load path where the app
// fetches `full.json` (canonical shape) for the first time.
test.use({ storageState: { cookies: [], origins: [] } })

// Name used by the save-as flow assertion below. Kept off the shared
// playwright_test_profile name so the bootstrap test never collides
// with the rest of the suite.
const SAVE_AS_TEST_NAME = `e2e_saveas_${Date.now()}`

test.afterEach(async () => {
  // Best-effort cleanup of the working/ entry the save-as test creates.
  // Using a fresh API context — the test's own page context is gone by now.
  const apiContext = await playwrightRequest.newContext({
    baseURL: 'http://localhost:3000',
  })
  try {
    const resp = await apiContext.delete(`/api/profiles/${SAVE_AS_TEST_NAME}`)
    if (!resp.ok() && resp.status() !== 404) {
      console.warn(`Cleanup of ${SAVE_AS_TEST_NAME} returned ${resp.status()}`)
    }
  } finally {
    await apiContext.dispose()
  }
})

test('full.json loads in browser without errors', async ({ page }) => {
  const errors: string[] = []
  const consoleErrors: string[] = []

  page.on('pageerror', (e) =>
    errors.push(`pageerror: ${e.message}\n${e.stack ?? ''}`),
  )
  page.on('console', (msg) => {
    if (msg.type() === 'error') {
      consoleErrors.push(`console.error: ${msg.text()}`)
    }
  })

  await page.goto('/')

  // Header renders even before profile loads.
  await expect(page.getByText(/Current Profile:/i)).toBeVisible({
    timeout: 15000,
  })

  // Wait for the bootstrap-loaded profile name (full.json default) to appear.
  // Header renders as "Current Profile: full" — match on the standalone token.
  await expect(page.getByText(/Current Profile:\s*full\b/i)).toBeVisible({
    timeout: 15000,
  })

  // Give async error handlers a moment to fire.
  await page.waitForTimeout(2000)

  // Click through each tab to exercise the canonical sub-struct rendering.
  for (const tabName of [
    'Entities',
    'Binary Outputs',
    'Binary Inputs',
    'Analog Outputs',
    'Analog Inputs',
    'Curves',
    'Scheduling',
  ]) {
    const tab = page.getByRole('tab', { name: tabName })
    if ((await tab.count()) > 0) {
      await tab.first().click()
      await page.waitForTimeout(500)
    }
  }

  // --------------------------------------------------------------------
  // Exercise modal-rendering paths so console errors from Dialog
  // primitives (forwardRef, missing Description) are caught here.
  // The error/console listeners above will trip the assertion at the
  // bottom if any modal render emits a warning.
  // --------------------------------------------------------------------

  // Load Profile modal — opened by clicking the header button.
  const openLoad = page.getByRole('button', { name: /^load profile$/i }).first()
  if ((await openLoad.count()) > 0) {
    await openLoad.click()
    await expect(page.getByRole('dialog')).toBeVisible({ timeout: 5000 })
    await page.keyboard.press('Escape')
    await page.waitForTimeout(300)
  }

  // Preferences modal.
  const openPrefs = page.getByRole('button', { name: /^preferences$/i }).first()
  if ((await openPrefs.count()) > 0) {
    await openPrefs.click()
    await expect(page.getByRole('dialog')).toBeVisible({ timeout: 5000 })
    await page.keyboard.press('Escape')
    await page.waitForTimeout(300)
  }

  // SaveAs modal — Save button is disabled until isModified=true. Land
  // on Entities, bump a NumberInput to dirty the profile, then trigger
  // Save. With loadedSource='seed' (full.json bootstrap), Save routes
  // to the SaveAs modal.
  const entitiesTab = page.getByRole('tab', { name: 'Entities' })
  if ((await entitiesTab.count()) > 0) {
    await entitiesTab.first().click()
    await page.waitForTimeout(300)

    // Grab the first NumberInput on the Entities tab and nudge it up
    // by pressing ArrowUp once after focusing. NumberInput is a custom
    // component but it wraps an <input type="number">.
    const firstNumberInput = page.locator('input[type="number"]').first()
    if ((await firstNumberInput.count()) > 0) {
      await firstNumberInput.focus()
      await firstNumberInput.press('ArrowUp')
      // Blur to commit the change.
      await firstNumberInput.press('Tab')
      await page.waitForTimeout(300)

      const saveButton = page.getByRole('button', { name: /^save$/i }).first()
      if ((await saveButton.count()) > 0 && (await saveButton.isEnabled())) {
        await saveButton.click()
        await expect(page.getByRole('dialog')).toBeVisible({ timeout: 5000 })
        await page.keyboard.press('Escape')
        await page.waitForTimeout(300)
      }
    }
  }

  if (errors.length > 0 || consoleErrors.length > 0) {
    const summary = [...errors, ...consoleErrors].join('\n\n')
    throw new Error(`Browser errors during full.json bootstrap:\n${summary}`)
  }
})

test('save-as flow updates header, dirty flag, and profile list', async ({
  page,
}) => {
  // Fresh boot: load full (profile, modify it, save-as under a new name,
  // and assert every state transition the user can observe:
  //   1. SaveAsModal closes on success
  //   2. Header flips from "full" to the new name
  //   3. Save button disables (isModified=false after save)
  //   4. localStorage.mesa.lastLoadedProfile tracks the new name
  //   5. Reopening Load Profile shows the new entry under "My Profiles"
  await page.goto('/')

  // Wait for bootstrap to settle on full.
  await expect(page.getByText(/Current Profile:\s*full\b/i)).toBeVisible({
    timeout: 15000,
  })

  // Got to Entities and modify the first NumberInput.
  await page.getByRole('tab', { name: 'Entities' }).first().click()

  const firstNumberInput = page.locator('input[type="number"]').first()
  await firstNumberInput.focus()
  await firstNumberInput.press('ArrowUp')
  await firstNumberInput.press('Tab')

  // Save → SaveAsModal opens (loadedSource=seed forces the branch).
  const saveButton = page.getByRole('button', { name: /^save$/i }).first()
  await expect(saveButton).toBeEnabled()
  await saveButton.click()

  const dialog = page.getByRole('dialog')
  await expect(dialog).toBeVisible({ timeout: 5000 })
  await expect(dialog.getByText(/Save Profile As/i)).toBeVisible()

  // Type the new name and submit.
  const nameInput = dialog.getByLabel('Profile name')
  await nameInput.fill(SAVE_AS_TEST_NAME)
  await dialog.getByRole('button', { name: /^save$/i }).click()

  // 1. SaveAsModal closes.
  await expect(dialog).toBeHidden({ timeout: 5000 })

  // Toast confirms the save (also covers the showToast path).
  await expect(
    page.getByText(`Profile "${SAVE_AS_TEST_NAME}.json" saved successfully!`),
  ).toBeVisible({ timeout: 5000 })

  // 2. Header flips from "full" to the new name. Match on a regex
  // anchored to the label so we don't false-positive on partial text.
  await expect(
    page.getByText(new RegExp(`Current Profile:\\s*${SAVE_AS_TEST_NAME}\\b`)),
  ).toBeVisible({ timeout: 5000 })
  await expect(page.getByText(/Current Profile:\s*full\b/)).toHaveCount(0)

  // 3. Save button disables — isModified should reset on save.
  await expect(saveButton).toBeDisabled()

  // 4. localStorage anchor tracks the new name.
  const lastLoaded = await page.evaluate(() =>
    localStorage.getItem('mesa.lastLoadedProfile'),
  )
  expect(lastLoaded).toBe(SAVE_AS_TEST_NAME)

  // 5. Reopen Load Profile — new entry shows under "My Profiles" and
  // the modal re-fetches on each open (no stale cache).
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
  await expect(myProfilesSection.getByText(SAVE_AS_TEST_NAME)).toBeVisible({
    timeout: 5000,
  })

  await page.keyboard.press('Escape')
  await expect(loadDialog).toBeHidden()
})
