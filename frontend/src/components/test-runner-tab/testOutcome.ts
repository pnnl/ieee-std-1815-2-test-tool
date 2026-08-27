/**
 * Shared test outcome derivation.
 *
 * This is the single source of truth for how a test result maps to a
 * pass/fail/pending outcome. Both the visual tree (TestResultsTree) and the
 * export builder (exportBundle) call this function so they agree by
 * construction and can never drift.
 *
 * Logic (mirrors the on-screen tree):
 *   - No result recorded yet (undefined): pending.
 *   - passed=true  and shouldPass=true:  passed.
 *   - passed=true  and shouldPass=false: failed  (expected-to-fail, but passed).
 *   - passed=false and shouldPass=true:  failed.
 *   - passed=false and shouldPass=false: passed  (expected to fail, and did).
 *   - passed=null (tri-state): pending (result received but indeterminate).
 */

export type TestOutcome = 'passed' | 'failed' | 'pending'

/**
 * Derive the outcome for a single test.
 *
 * @param passed      The `passed` field from ConformanceTestResult, or
 *                    undefined when no result has arrived yet.
 * @param shouldPass  Whether the test is expected to pass per the scenario spec.
 */
export function deriveOutcome(
  passed: boolean | null | undefined,
  shouldPass: boolean,
): TestOutcome {
  if (passed === undefined) {
    // No result recorded yet: pending.
    return 'pending'
  }
  if (passed === null) {
    // Tri-state null: result arrived but indeterminate.
    return 'pending'
  }
  // Boolean result: XOR with shouldPass determines pass/fail.
  const isPass = passed === shouldPass
  return isPass ? 'passed' : 'failed'
}
