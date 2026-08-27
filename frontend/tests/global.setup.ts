import { test as setup, request } from '@playwright/test'
import { TEST_PROFILE_NAME, PW_BASE_URL } from './shared_config'

export const STORAGE_STATE_PATH = 'storage-state.json'

setup('create test profile', async ({ page }) => {
  const apiContext = await request.newContext({ baseURL: PW_BASE_URL })
  try {
    // Fetch the canonical `full` seed via the API (legacy template was
    // removed in the canonical migration).
    const seedResponse = await apiContext.get('/api/profiles/full')
    if (!seedResponse.ok()) {
      throw new Error(
        `Failed to fetch canonical full seed: ${seedResponse.status()}`,
      )
    }
    const templateData = await seedResponse.json()

    const saveResponse = await apiContext.post('/api/profiles', {
      data: { name: TEST_PROFILE_NAME, profile: templateData },
    })
    if (!saveResponse.ok()) {
      throw new Error(`Failed to create test profile: ${saveResponse.status()}`)
    }

    // Navigate to the app and select the profile via localStorage so tests start with it loaded
    await page.goto('/')
    await page.evaluate((profileName) => {
      localStorage.setItem('mesa.lastLoadedProfile', profileName)
    }, TEST_PROFILE_NAME)

    await page.context().storageState({ path: STORAGE_STATE_PATH })
  } finally {
    await apiContext.dispose()
  }
})
