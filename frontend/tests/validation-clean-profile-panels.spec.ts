import { test, expect } from '@playwright/test'

// Uses the read-only canonical `full` seed directly: nothing is created,
// mutated, or saved, so there is nothing to tear down. Proves the
// rendering half of the invariant: an empty error bucket renders no
// callout shell, on every tab including BO/BI, which have no backend
// validation producer.

test.use({ storageState: { cookies: [], origins: [] } })

test('a clean profile shows no General panel and no BO/BI callout shell', async ({
  page,
}) => {
  await page.goto('/')

  await expect(page.getByText(/Current Profile:\s*full\b/i)).toBeVisible({
    timeout: 15000,
  })

  // The General panel sits above the tab system; it must not render for
  // a clean profile.
  await expect(page.getByText('Profile validation issues')).toHaveCount(0)
  await expect(page.locator('.rt-CalloutRoot')).toHaveCount(0)

  for (const tabName of ['Binary Outputs', 'Binary Inputs'] as const) {
    await page.getByRole('tab', { name: tabName }).click()

    // Mount the Scada section so its rows are actually in the DOM.
    const scadaButton = page.getByRole('button', { name: /\bScada\b/i }).first()
    await expect(scadaButton).toBeVisible()
    await scadaButton.click()
    const firstDataRow = page.getByRole('row').nth(1)
    await expect(firstDataRow).toBeVisible()

    // No empty callout shell and no zero-count badge: BO/BI have no
    // backend validation producer, so absence is the only correct
    // rendering.
    await expect(page.locator('.rt-CalloutRoot')).toHaveCount(0)
    await expect(page.getByText(/\d+\s+problems?/i)).toHaveCount(0)
  }
})
