import type { TestName } from '@/api/generated'

type ExpectedTest = {
  test_id: TestName
  should_pass: boolean
}

export type Scenario = {
  id: string
  name: string
  description: string
  expected_tests: ExpectedTest[]
}

export async function fetchScenarios(): Promise<Scenario[]> {
  const response = await fetch('/api/scenarios')
  if (!response.ok) {
    throw new Error('Failed to fetch scenarios')
  }
  return response.json()
}
