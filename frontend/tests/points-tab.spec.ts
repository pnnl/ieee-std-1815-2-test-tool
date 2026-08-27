import { test, expect } from '@playwright/test'
import { TEST_PROFILE_NAME } from './shared_config'

const POINT_TABS = [
  'Binary Outputs',
  'Binary Inputs',
  'Analog Outputs',
  'Analog Inputs',
] as const

for (const tabName of POINT_TABS) {
  test(`${tabName} tab: SCADA section exists and shows points after expanding`, async ({
    page,
  }) => {
    await page.goto('/')

    // Wait for the profile to auto-load from localStorage. Bootstrap is silent
    // (no toast on auto-load), so observe the header profile name instead.
    await expect(page.getByText(TEST_PROFILE_NAME)).toBeVisible({
      timeout: 10000,
    })

    await page.getByRole('tab', { name: tabName }).click()

    // Locate the Scada section header button
    const scadaButton = page.getByRole('button', { name: /\bScada\b/i }).first()
    await expect(scadaButton).toBeVisible()

    // Expand the section
    await scadaButton.click()

    // After expansion, at least one table row should be visible
    const firstRow = page.getByRole('row').first()
    await expect(firstRow).toBeVisible()
  })
}
