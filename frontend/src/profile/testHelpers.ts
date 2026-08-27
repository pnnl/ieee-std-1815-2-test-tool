// Shared test fixture builder for the profile-module tests.

import type { ValidationError } from '@/api/generated'

export function err(
  point: string,
  message = `message for ${point}`,
): ValidationError {
  return { point, message }
}
