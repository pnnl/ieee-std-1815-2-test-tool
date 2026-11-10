import { useMemo } from 'react'
import { Check, Minus, X } from 'lucide-react'
import type { PicsProfile } from '@/api/generated'
import { buildPointResults, POINT_CHECKS } from './pointResults'
import type { ConformanceTestResult } from './types'
import styles from './PointResultsTable.module.css'

type PointResultsTableProps = {
  profile: PicsProfile
  results: Record<string, ConformanceTestResult>
}

export default function PointResultsTable({
  profile,
  results,
}: PointResultsTableProps) {
  const rows = useMemo(
    () => buildPointResults(profile, results),
    [profile, results],
  )

  return (
    <section className={styles.results} aria-labelledby="point-results-title">
      <h2 id="point-results-title" className={styles.title}>
        Point Results
      </h2>
      <div className={styles.scrollArea} tabIndex={0}>
        <table className={styles.table}>
          <thead>
            <tr>
              <th scope="col">Point</th>
              <th scope="col">Name</th>
              {POINT_CHECKS.map(({ check, label }) => (
                <th scope="col" key={check}>
                  {label}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {rows.map((row) => (
              <tr key={row.index}>
                <th scope="row">{row.index}</th>
                <td>{row.name}</td>
                {POINT_CHECKS.map(({ check, label }) => {
                  const result = row.checks[check]
                  let outcome = 'No information'
                  let icon = (
                    <Minus
                      className={styles.unknown}
                      size={18}
                      aria-hidden="true"
                    />
                  )
                  if (result?.passed) {
                    outcome = 'Passed'
                    icon = (
                      <Check
                        className={styles.passed}
                        size={18}
                        aria-hidden="true"
                      />
                    )
                  } else if (result) {
                    outcome = 'Failed'
                    icon = (
                      <X
                        className={styles.failed}
                        size={18}
                        aria-hidden="true"
                      />
                    )
                  }
                  const details = result
                    ? `. ${result.messages.join('. ')}`
                    : ''
                  return (
                    <td
                      key={check}
                      className={styles.resultCell}
                      aria-label={`${row.index}, ${label}: ${outcome}${details}`}
                      title={result?.messages.join('\n')}
                    >
                      {icon}
                    </td>
                  )
                })}
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </section>
  )
}
