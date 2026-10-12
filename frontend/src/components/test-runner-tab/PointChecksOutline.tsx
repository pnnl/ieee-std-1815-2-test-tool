import { Fragment, useMemo, useState, type ReactNode } from 'react'
import { Check, X } from 'lucide-react'
import type { PointCheckResult } from './types'
import styles from './PointChecksOutline.module.css'

type Expansion = {
  isExpanded: (id: string) => boolean
  onToggle: (id: string) => void
}

function Disclosure({
  id,
  label,
  children,
  isExpanded,
  onToggle,
}: Expansion & { id: string; label: ReactNode; children: ReactNode }) {
  const open = isExpanded(id)
  return (
    <div className={styles.section}>
      <button
        type="button"
        aria-expanded={open}
        onClick={() => onToggle(id)}
        className={styles.disclosure}
      >
        <span aria-hidden="true">{open ? '▾' : '▸'}</span>
        <span>{label}</span>
      </button>
      {open && <div className={styles.children}>{children}</div>}
    </div>
  )
}

type PointChecks = readonly [string, readonly PointCheckResult[]]

function pointIndex(check: PointCheckResult) {
  return `${check.point.point_type}${check.point.index}`
}

function checkKey(check: PointCheckResult) {
  return `${pointIndex(check)}:${check.check_id}`
}

function CheckTable({ points }: { points: readonly PointChecks[] }) {
  const [details, setDetails] = useState<Record<string, boolean>>({})
  const showDetails = (uid: string, check: PointCheckResult) =>
    details[`${uid}:${checkKey(check)}`] ?? !check.passed
  const columns = useMemo(() => {
    const labels = new Map<string, string>()
    for (const [, checks] of points) {
      for (const check of checks)
        labels.set(check.check_id, check.expected_behavior)
    }
    const order = [
      'profile_definition',
      'value_received',
      'write_accepted',
      'value_in_range',
    ]
    const rank = (id: string) => {
      const position = order.indexOf(id)
      return position === -1 ? order.length : position
    }
    return [...labels.entries()].sort(([a], [b]) => rank(a) - rank(b))
  }, [points])

  return (
    <div className={styles.tableContainer}>
      <table className={styles.table} aria-label="Point tests">
        <thead>
          <tr>
            <th scope="col">Point</th>
            {columns.map(([id, label]) => (
              <th key={id} scope="col" className={styles.result}>
                {label}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {points.map(([uid, checks]) => {
            const point = checks[0].point
            // Profile names sometimes include enumeration documentation after the name.
            const name =
              point.name?.split(/\.\s+|\r?\n/, 1)[0].trim() || point.uid
            const indices = [...new Set(checks.map(pointIndex))].join(', ')
            const visibleDetails = checks.filter((check) =>
              showDetails(uid, check),
            )
            return (
              <Fragment key={uid}>
                <tr>
                  <th scope="row" className={styles.pointCell}>
                    <span className={styles.pointHeader}>
                      <strong>{name}</strong>
                      <span className={styles.uid}>{point.uid}</span>
                      <span className={styles.pointIndex}>{indices}</span>
                    </span>
                  </th>
                  {columns.map(([id]) => {
                    const cellChecks = checks.filter(
                      (check) => check.check_id === id,
                    )
                    if (cellChecks.length === 0)
                      return (
                        <td key={id} className={styles.result}>
                          —
                        </td>
                      )
                    return (
                      <td key={id} className={styles.result}>
                        <span className={styles.resultChecks}>
                          {cellChecks.map((check) => {
                            const index = pointIndex(check)
                            const key = `${uid}:${checkKey(check)}`
                            const expanded = showDetails(uid, check)
                            return (
                              <button
                                key={key}
                                type="button"
                                className={styles.resultButton}
                                data-passed={check.passed}
                                aria-label={`${index}: ${check.expected_behavior}: ${check.passed ? 'Passed' : 'Failed'}. ${expanded ? 'Hide' : 'Show'} details`}
                                aria-expanded={expanded}
                                title={`${index}: ${check.message}`}
                                onClick={() =>
                                  setDetails((previous) => ({
                                    ...previous,
                                    [key]: !expanded,
                                  }))
                                }
                              >
                                {cellChecks.length > 1 && <span>{index}</span>}
                                {check.passed ? (
                                  <Check
                                    size={16}
                                    strokeWidth={3}
                                    aria-hidden="true"
                                  />
                                ) : (
                                  <X
                                    size={16}
                                    strokeWidth={3}
                                    aria-hidden="true"
                                  />
                                )}
                              </button>
                            )
                          })}
                        </span>
                      </td>
                    )
                  })}
                </tr>
                {visibleDetails.length > 0 && (
                  <tr>
                    <td
                      colSpan={columns.length + 1}
                      className={styles.evidence}
                    >
                      {visibleDetails.map((check) => (
                        <div key={checkKey(check)}>
                          <strong>
                            {pointIndex(check)}: {check.expected_behavior}:
                          </strong>{' '}
                          {check.message}
                        </div>
                      ))}
                    </td>
                  </tr>
                )}
              </Fragment>
            )
          })}
        </tbody>
      </table>
    </div>
  )
}

export default function PointChecksOutline({
  checks,
  groupId,
  label,
  ...expansion
}: Expansion & {
  checks: PointCheckResult[]
  groupId: string
  label: ReactNode
}) {
  const points = useMemo(() => {
    const grouped = new Map<string, Map<string, PointCheckResult>>()
    for (const check of [...checks].sort((a, b) => a.sequence - b.sequence)) {
      const key = check.point.uid
      const existing = grouped.get(key) ?? new Map<string, PointCheckResult>()
      existing.set(checkKey(check), check)
      grouped.set(key, existing)
    }
    return [...grouped.entries()].map(
      ([key, tests]) => [key, [...tests.values()]] as const,
    )
  }, [checks])

  return (
    <div
      onClick={(event) => event.stopPropagation()}
      onKeyDown={(event) => event.stopPropagation()}
    >
      <Disclosure id={groupId} label={label} {...expansion}>
        {points.length === 0 ? (
          <p className={styles.empty}>No point check results received yet.</p>
        ) : (
          <CheckTable points={points} />
        )}
      </Disclosure>
    </div>
  )
}
