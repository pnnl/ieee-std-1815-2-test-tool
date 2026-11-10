import { useState } from 'react'
import { ArrowDown, ArrowUp, ArrowUpDown, Check, Minus, X } from 'lucide-react'
import type { PicsProfile } from '@/api/generated'
import {
  buildPointResults,
  POINT_CHECKS,
  type PointResult,
  type PointResultRow,
} from './pointResults'
import type { ConformanceTestResult } from './types'
import styles from './PointResultsTable.module.css'

type PointResultsTableProps = {
  profile: PicsProfile
  results: Record<string, ConformanceTestResult>
}

type CheckColumn = (typeof POINT_CHECKS)[number]['check']
type SortColumn = 'index' | 'name' | 'status' | CheckColumn

function resultRank(result: PointResult | undefined): number {
  if (!result) return 1
  return result.passed ? 2 : 0
}

function isNotApplicable(row: PointResultRow, check: CheckColumn): boolean {
  return (
    (check === 'written_to_outstation' && /^(AI|BI)\d+$/.test(row.index)) ||
    (check === 'received_from_outstation' && /^(AO|BO)\d+$/.test(row.index)) ||
    (check === 'within_profile_range' &&
      (row.index.startsWith('BI') || row.index.startsWith('BO'))) ||
    (check === 'curve_readback_matches_profile' &&
      !row.curveReadbackApplicable) ||
    (check === 'schedule_readback_matches_profile' &&
      !row.scheduleReadbackApplicable)
  )
}

function statusRank(row: PointResultRow): number {
  return Math.min(
    ...POINT_CHECKS.filter(({ check }) => !isNotApplicable(row, check)).map(
      ({ check }) => resultRank(row.checks[check]),
    ),
  )
}

export default function PointResultsTable({
  profile,
  results,
}: PointResultsTableProps) {
  const [sort, setSort] = useState<{
    column: SortColumn
    direction: 'ascending' | 'descending'
  }>({ column: 'status', direction: 'ascending' })
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
      } else if (sort.column === 'status') {
        comparison = statusRank(first) - statusRank(second)
      } else {
        const firstResult = isNotApplicable(first, sort.column)
          ? undefined
          : first.checks[sort.column]
        const secondResult = isNotApplicable(second, sort.column)
          ? undefined
          : second.checks[sort.column]
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
              {renderHeader('status', 'Status')}
              {POINT_CHECKS.map(({ check, label }) =>
                renderHeader(check, label),
              )}
            </tr>
          </thead>
          <tbody>
            {rows.map((row) => {
              const status = statusRank(row)
              let statusIcon = (
                <Minus
                  className={styles.unknown}
                  size={18}
                  aria-hidden="true"
                />
              )
              if (status === 0) {
                statusIcon = (
                  <X className={styles.failed} size={18} aria-hidden="true" />
                )
              } else if (status === 2) {
                statusIcon = (
                  <Check
                    className={styles.passed}
                    size={18}
                    aria-hidden="true"
                  />
                )
              }
              return (
                <tr key={row.index}>
                  <th scope="row">{row.index}</th>
                  <td>{row.name}</td>
                  <td
                    className={styles.resultCell}
                    aria-label={`${row.index}, Status: ${['Failed', 'No information', 'Passed'][status]}`}
                  >
                    {statusIcon}
                  </td>
                  {POINT_CHECKS.map(({ check, label }) => {
                    const notApplicable = isNotApplicable(row, check)
                    const result = notApplicable ? undefined : row.checks[check]
                    let outcome = 'No information'
                    let icon = (
                      <Minus
                        className={styles.unknown}
                        size={18}
                        aria-hidden="true"
                      />
                    )
                    if (notApplicable) {
                      outcome = 'Not applicable'
                      icon = <>n/a</>
                    } else if (result?.passed) {
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
              )
            })}
          </tbody>
        </table>
      </div>
    </section>
  )
}
