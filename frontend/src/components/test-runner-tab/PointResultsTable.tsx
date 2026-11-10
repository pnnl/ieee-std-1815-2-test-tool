import { useState } from 'react'
import { ArrowDown, ArrowUp, ArrowUpDown, Check, Minus, X } from 'lucide-react'
import type { PicsProfile } from '@/api/generated'
import {
  buildPointResults,
  POINT_CHECKS,
  type PointResult,
} from './pointResults'
import type { ConformanceTestResult } from './types'
import styles from './PointResultsTable.module.css'

type PointResultsTableProps = {
  profile: PicsProfile
  results: Record<string, ConformanceTestResult>
}

type SortColumn = 'index' | 'name' | (typeof POINT_CHECKS)[number]['check']

function resultRank(result: PointResult | undefined): number {
  if (!result) return 1
  return result.passed ? 2 : 0
}

export default function PointResultsTable({
  profile,
  results,
}: PointResultsTableProps) {
  const [sort, setSort] = useState<{
    column: SortColumn
    direction: 'ascending' | 'descending'
  } | null>(null)
  const rows = buildPointResults(profile, results)

  if (sort) {
    const compareText = (first: string, second: string) =>
      first.localeCompare(second, undefined, {
        numeric: true,
        sensitivity: 'base',
      })
    rows.sort((first, second) => {
      let comparison: number
      if (sort.column === 'index' || sort.column === 'name') {
        comparison = compareText(first[sort.column], second[sort.column])
      } else {
        const firstResult = first.checks[sort.column]
        const secondResult = second.checks[sort.column]
        comparison = resultRank(firstResult) - resultRank(secondResult)
      }
      if (comparison === 0) return compareText(first.index, second.index)
      return sort.direction === 'ascending' ? comparison : -comparison
    })
  }

  const renderHeader = (column: SortColumn, label: string) => {
    const active = sort?.column === column
    let icon = <ArrowUpDown size={14} aria-hidden="true" />
    if (active && sort.direction === 'ascending') {
      icon = <ArrowUp size={14} aria-hidden="true" />
    } else if (active) {
      icon = <ArrowDown size={14} aria-hidden="true" />
    }
    return (
      <th
        scope="col"
        key={column}
        aria-sort={active ? sort.direction : undefined}
      >
        <button
          type="button"
          className={styles.sortButton}
          aria-label={`Sort by ${label}`}
          onClick={() =>
            setSort((previous) => ({
              column,
              direction:
                previous?.column === column &&
                previous.direction === 'ascending'
                  ? 'descending'
                  : 'ascending',
            }))
          }
        >
          {label} {icon}
        </button>
      </th>
    )
  }

  return (
    <section className={styles.results} aria-labelledby="point-results-title">
      <h2 id="point-results-title" className={styles.title}>
        Point Results
      </h2>
      <div className={styles.scrollArea} tabIndex={0}>
        <table className={styles.table}>
          <thead>
            <tr>
              {renderHeader('index', 'Point')}
              {renderHeader('name', 'Name')}
              {POINT_CHECKS.map(({ check, label }) =>
                renderHeader(check, label),
              )}
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
