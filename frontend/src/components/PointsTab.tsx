import { useMemo } from 'react'
import { Badge } from '@/components/ui/badge'
import { MultiSelect } from '@/components/ui/multi-select'
import OffsetSection from './OffsetSection'
import ValidationErrorCallout from './ValidationErrorCallout'
import {
  flattenSection,
  SECTION_OFFSETS,
  type PointSection,
  type FlatPoint,
} from '@/profile/canonical'
import {
  groupPointsByOffset,
  groupErrorCountsByOffset,
  groupErrorsByPointIndex,
} from '@/profile/offsetBands'
import type { PicsProfile, ValidationError } from '@/api/generated'

type EditableField =
  | 'name'
  | 'mandatory_1815'
  | 'mandatory_1547'
  | 'value'
  | 'purpose'
  | 'units'
  | 'iec_61850_uid'

interface PointsTabProps {
  tabName: PointSection
  profileData: PicsProfile
  onPointUpdate: (
    point: FlatPoint,
    field: EditableField,
    value: string | number | boolean | null,
  ) => void
  purposeFilter: string[]
  onPurposeFilterChange: (value: string[]) => void
  autoExpandSections?: boolean
  // Validation errors bucketed to this tab. BO/BI currently have no
  // backend validation, so this is routinely empty for them; the callout
  // renders nothing when empty, so an unchecked tab never implies a problem.
  errors: ValidationError[]
}

function PointsTab({
  tabName,
  profileData,
  onPointUpdate,
  purposeFilter,
  onPurposeFilterChange,
  autoExpandSections,
  errors,
}: PointsTabProps) {
  const allPoints = useMemo(
    () => flattenSection(profileData, tabName),
    [profileData, tabName],
  )

  const purposeOptions = useMemo(() => {
    const counts = new Map<string, number>()
    for (const p of allPoints) {
      if (p.purpose) counts.set(p.purpose, (counts.get(p.purpose) ?? 0) + 1)
    }
    return Array.from(counts.entries())
      .sort((a, b) => a[0].localeCompare(b[0]))
      .map(([purpose, count]) => ({
        value: purpose,
        label: `${purpose} (${count})`,
      }))
  }, [allPoints])

  const filteredPoints = useMemo(() => {
    if (purposeFilter.length === 0) return allPoints
    return allPoints.filter(
      (p) => p.purpose && purposeFilter.includes(p.purpose),
    )
  }, [allPoints, purposeFilter])

  const groupedPoints = useMemo(
    () => groupPointsByOffset(filteredPoints),
    [filteredPoints],
  )

  const visiblePointCount = useMemo(
    () =>
      Object.values(groupedPoints).reduce(
        (sum, points) => sum + points.length,
        0,
      ),
    [groupedPoints],
  )

  // Counted off the full, unfiltered `errors` prop, same source the tab
  // badge reads from, so the purpose filter can't desync the two badges.
  const groupErrorCounts = useMemo(
    () => groupErrorCountsByOffset(errors),
    [errors],
  )

  // A POINTS lookup (one entry per errored point index), distinct from the
  // ERRORS count in `groupErrorCounts` above; see offsetBands.ts for why
  // the two numbers don't reconcile.
  const errorsByPointIndex = useMemo(
    () => groupErrorsByPointIndex(errors),
    [errors],
  )

  return (
    <div className="flex flex-col gap-4 p-4">
      <ValidationErrorCallout errors={errors} />

      <div className="flex items-end justify-between">
        <h3 className="text-lg font-semibold">{formatTabName(tabName)}</h3>
        <div className="flex items-end gap-2">
          <MultiSelect
            label="Filter by Purpose"
            placeholder="All purposes..."
            options={purposeOptions}
            selected={purposeFilter}
            onChange={onPurposeFilterChange}
            className="min-w-[220px]"
          />
          <Badge
            variant="secondary"
            className="text-sm px-3 py-1 whitespace-nowrap"
          >
            {visiblePointCount} of {allPoints.length} points
          </Badge>
        </div>
      </div>

      {Object.entries(SECTION_OFFSETS)
        .sort((a, b) => a[1] - b[1])
        .map(([offsetName, offsetValue]) => {
          const points = groupedPoints[offsetName] || []
          if (points.length === 0) return null

          return (
            <OffsetSection
              key={offsetName}
              offsetName={offsetName}
              offsetValue={offsetValue}
              points={points}
              tabName={tabName}
              onPointUpdate={onPointUpdate}
              autoExpand={autoExpandSections}
              errorCount={groupErrorCounts[offsetName] ?? 0}
              errorsByPointIndex={errorsByPointIndex}
            />
          )
        })}
    </div>
  )
}

function formatTabName(tabName: string): string {
  return tabName
    .split('_')
    .map((word) => word.charAt(0).toUpperCase() + word.slice(1))
    .join(' ')
}

export default PointsTab
