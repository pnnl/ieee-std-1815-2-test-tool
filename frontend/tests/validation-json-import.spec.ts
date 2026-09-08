import { test, expect, request as playwrightRequest } from '@playwright/test'
import { PW_BASE_URL } from './shared_config'

// Covers the JSON import path end to end against the real backend: a
// semantically invalid profile loads with its error routed to the owning
// point tab and never leaks into the General panel, and a clean profile
// loads with no panel or callout rendered at all.
//
// Two fixture profiles are built from the canonical `full` seed and saved
// under unique, timestamped names so this file never collides with another
// spec's fixture data. Both are torn down in afterAll.

const AI_ERROR_PROFILE = `e2e_492_ai_error_${Date.now()}`
const CLEAN_IMPORT_PROFILE = `e2e_492_clean_import_${Date.now()}`

test.describe('JSON import surfaces validation errors', () => {
  // Both tests share the beforeAll-built fixtures; force serial execution
  // so afterAll's cleanup can't run while the other test is mid-flight.
  test.describe.configure({ mode: 'serial' })

  let aiErrorProfileBody: Record<string, unknown>
  let cleanProfileBody: Record<string, unknown>
  let aiPointIndex: number

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
      cleanProfileBody = seed

      if (!Array.isArray(seed.AI?.points) || seed.AI.points.length === 0) {
        throw new Error(
          'Canonical seed carries no AI points to build the fixture from.',
        )
      }

      // Deep copy so mutating the fixture never touches cleanProfileBody.
      const mutated = JSON.parse(JSON.stringify(seed))
      const firstAiPoint = mutated.AI.points[0]
      aiPointIndex = firstAiPoint.point_index

      // Force the "minimum > maximum" check with a large, unambiguous
      // offset, regardless of the seed's original values.
      firstAiPoint.minimum = firstAiPoint.maximum + 100000
      aiErrorProfileBody = mutated
    } finally {
      await apiContext.dispose()
    }
  })

  test.afterAll(async () => {
    const apiContext = await playwrightRequest.newContext({
      baseURL: PW_BASE_URL,
    })
    try {
      for (const name of [AI_ERROR_PROFILE, CLEAN_IMPORT_PROFILE]) {
        const resp = await apiContext.delete(`/api/profiles/${name}`)
        if (!resp.ok() && resp.status() !== 404) {
          console.warn(`Cleanup of ${name} returned ${resp.status()}`)
        }
      }
    } finally {
      await apiContext.dispose()
    }
  })

  test('obligation 13: an AI semantic error routes to Analog Inputs, not General, and the profile loads', async ({
    page,
  }) => {
    // The file doesn't exist yet, so handleImportFile takes the prompt()
    // branch. Re-supplying the default value keeps the saved name equal
    // to the imported file's base name.
    page.on('dialog', (dialog) => {
      void dialog.accept(dialog.defaultValue())
    })

    await page.goto('/')
    await expect(page.getByText(/Current Profile:/i)).toBeVisible({
      timeout: 15000,
    })

    await page.locator('#file-input').setInputFiles({
      name: `${AI_ERROR_PROFILE}.json`,
      mimeType: 'application/json',
      buffer: Buffer.from(JSON.stringify(aiErrorProfileBody)),
    })

    // A JSON profile that fails validation still loads into the editor.
    //
    // 20s: the import path awaits three sequential requests, and CI's
    // single worker may still be finishing a prior spec's backend work.
    await expect(
      page.getByText(new RegExp(`Current Profile:\\s*${AI_ERROR_PROFILE}\\b`)),
    ).toBeVisible({ timeout: 20000 })

    // Disjoint-surfaces invariant: this error never shows in General.
    await expect(page.getByText('Profile validation issues')).toHaveCount(0)

    // Bumping `minimum` above `maximum` also trips the eng_minimum/value
    // check for the same point, so two list items start with "AI0:": a
    // bare getByText('AI0:') is a strict-mode violation, filter by the
    // full message text instead.
    await page.getByRole('tab', { name: 'Analog Inputs' }).click()
    const minimumError = page.getByRole('listitem').filter({
      hasText: 'Minimum value cannot be greater than maximum value',
    })
    await expect(minimumError).toBeVisible()
    await expect(minimumError).toContainText(`AI${aiPointIndex}:`)
  })

  test('obligation 14: a clean JSON import loads with no panel or callout', async ({
    page,
  }) => {
    // Each test gets its own page, so the dialog handler from the other
    // test doesn't carry over: an unhandled dialog auto-dismisses and
    // cancels the save-as prompt, so this must set its own handler.
    page.on('dialog', (dialog) => {
      void dialog.accept(dialog.defaultValue())
    })

    await page.goto('/')
    await expect(page.getByText(/Current Profile:/i)).toBeVisible({
      timeout: 15000,
    })

    await page.locator('#file-input').setInputFiles({
      name: `${CLEAN_IMPORT_PROFILE}.json`,
      mimeType: 'application/json',
      buffer: Buffer.from(JSON.stringify(cleanProfileBody)),
    })

    // 20s: three sequential requests on the import path, not two.
    await expect(
      page.getByText(
        new RegExp(`Current Profile:\\s*${CLEAN_IMPORT_PROFILE}\\b`),
      ),
    ).toBeVisible({ timeout: 20000 })

    await expect(page.getByText('Profile validation issues')).toHaveCount(0)
    // rt-CalloutRoot is the stable Callout.Root class; its absence proves
    // no empty shell rendered anywhere, not just that General is gone.
    await expect(page.locator('.rt-CalloutRoot')).toHaveCount(0)
  })
})
