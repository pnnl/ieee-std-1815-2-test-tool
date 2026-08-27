import { test as teardown, request } from '@playwright/test'
import { TEST_PROFILE_NAME, PW_BASE_URL } from './shared_config'

teardown('delete test profile', async () => {
  const apiContext = await request.newContext({ baseURL: PW_BASE_URL })

  try {
    const response = await apiContext.delete(
      `/api/profiles/${TEST_PROFILE_NAME}`,
    )
    if (!response.ok() && response.status() !== 404) {
      throw new Error(`Failed to delete test profile: ${response.status()}`)
    }
  } finally {
    await apiContext.dispose()
  }
})
