import { useState, useEffect } from 'react'
import ValidationErrorBadge from '@/components/ValidationErrorBadge'
import { Collapsible, CollapsibleContent } from '@/components/ui/collapsible'
import {
  TableHeader,
  TableBody,
  TableRow,
  TableHead,
} from '@/components/ui/table'
import PointRow from './PointRow'
import type { FlatPoint, PointSection } from '@/profile/canonical'
import type { ValidationError } from '@/api/generated'

const DEFAULT_COLUMN_WIDTHS = {
  index: 80,
  description: 300,
  uid: 250,
  associatedIndex: 100,
  purpose: 150,
  mandatory: 100,
  ieee1815: 80,
  units: 100,
  value: 100,
}

type EditableField =
  | 'name'
  | 'mandatory_1815'
  | 'mandatory_1547'
  | 'value'
  | 'purpose'
  | 'units'
  | 'iec_61850_uid'

interface OffsetSectionProps {
  offsetName: string
  offsetValue: number
  points: FlatPoint[]
  tabName: PointSection
  onPointUpdate: (
    point: FlatPoint,
    field: EditableField,
    value: string | number | boolean | null,
  ) => void
  autoExpand?: boolean
  // Error COUNT, not point count: a point contributing two errors counts
  // as 2 here. Absent or zero renders no badge.
  errorCount?: number
  // Per-point error lookup for the row highlight. A point with no entry, or
  // an empty array, renders unstyled.
  errorsByPointIndex?: ReadonlyMap<number, readonly ValidationError[]>
}

function OffsetSection({
  offsetName,
  offsetValue,
  points,
  tabName,
  onPointUpdate,
  autoExpand,
  errorCount = 0,
  errorsByPointIndex,
}: OffsetSectionProps) {
  const [isExpanded, setIsExpanded] = useState(autoExpand || false)
  const [columnWidths, setColumnWidths] = useState<number[]>([])
  const isBinary = tabName.startsWith('binary')
  // Sub-option 3b: only AI points carry a `value`. Show the column only on
  // the AI tab.
  const showValueColumn = tabName === 'analog_inputs'

  // Header layout - "Mandatory" column was the legacy editor's
  // mandatory_1547 indicator and the IEEE 1815.2 column was mandatory_1815.
  // Both render directly off the canonical fields now.
  const headers: string[] = [
    'Point',
    'Name / Description',
    'IEC61850UniqueString',
    isBinary ? 'Assoc.' : 'Assoc.',
    'Purpose/Mode/Function',
    'Mandatory 1547',
    'Mandatory 1815',
  ]
  if (!isBinary) headers.push('Units')
  if (showValueColumn) headers.push('Value')

  useEffect(() => {
    if (autoExpand) setIsExpanded(true)
  }, [autoExpand])

  useEffect(() => {
    if (isExpanded && columnWidths.length === 0) {
      const widths = [
        DEFAULT_COLUMN_WIDTHS.index,
        DEFAULT_COLUMN_WIDTHS.description,
        DEFAULT_COLUMN_WIDTHS.uid,
        DEFAULT_COLUMN_WIDTHS.associatedIndex,
        DEFAULT_COLUMN_WIDTHS.purpose,
        DEFAULT_COLUMN_WIDTHS.mandatory,
        DEFAULT_COLUMN_WIDTHS.ieee1815,
      ]
      if (!isBinary) widths.push(DEFAULT_COLUMN_WIDTHS.units)
      if (showValueColumn) widths.push(DEFAULT_COLUMN_WIDTHS.value)
      setColumnWidths(widths)
    }
  }, [isExpanded, columnWidths.length, isBinary, showValueColumn])

  const handleMouseDown = (index: number) => (e: React.MouseEvent) => {
    e.preventDefault()
    e.stopPropagation()
    const startX = e.clientX
    const startWidth = columnWidths[index] || 100

    const handleMouseMove = (moveEvent: MouseEvent) => {
      const diff = moveEvent.clientX - startX
      const newWidth = Math.max(50, startWidth + diff)
      setColumnWidths((prev) => {
        const next = [...prev]
        next[index] = newWidth
        return next
      })
    }

    const handleMouseUp = () => {
      document.removeEventListener('mousemove', handleMouseMove)
      document.removeEventListener('mouseup', handleMouseUp)
    }

    document.addEventListener('mousemove', handleMouseMove)
    document.addEventListener('mouseup', handleMouseUp)
  }

  return (
    <div className="mb-1">
      <Collapsible open={isExpanded} onOpenChange={setIsExpanded}>
        <button
          onClick={() => setIsExpanded(!isExpanded)}
          className="w-full p-2 bg-muted rounded-sm text-left hover:bg-muted/80 transition-colors cursor-pointer"
        >
          <div className="flex items-center gap-2">
            <span className="text-sm font-bold font-mono">
              {isExpanded ? 'v' : '>'}
            </span>
            <span className="text-sm font-semibold">
              {formatOffsetName(offsetName)}
            </span>
            <span className="text-xs text-muted-foreground">
              Offset: {offsetValue} ({points.length} points)
            </span>
            {/* Outside CollapsibleContent so it stays visible while the
                group is collapsed, the exact case the badge exists for. */}
            {errorCount > 0 && <ValidationErrorBadge count={errorCount} />}
          </div>
        </button>

        <CollapsibleContent>
          <div className="relative w-full overflow-x-auto">
            <table
              className="w-full caption-bottom text-sm border"
              style={{ tableLayout: 'fixed' }}
            >
              <TableHeader className="sticky top-0 z-10 bg-background">
                <TableRow>
                  {headers.map((header, index) => (
                    <TableHead
                      key={header}
                      style={{
                        width: columnWidths[index]
                          ? `${columnWidths[index]}px`
                          : undefined,
                        position: 'relative',
                        userSelect: 'none',
                      }}
                    >
                      {header}
                      <div
                        onMouseDown={handleMouseDown(index)}
                        style={{
                          position: 'absolute',
                          right: 0,
                          top: 0,
                          bottom: 0,
                          width: 5,
                          cursor: 'col-resize',
                          zIndex: 1,
                        }}
                      />
                    </TableHead>
                  ))}
                </TableRow>
              </TableHeader>
              <TableBody>
                {points.map((point, idx) => (
                  <PointRow
                    key={`${offsetName}-${point.point_index}-${idx}`}
                    point={point}
                    isBinary={isBinary}
                    showValueColumn={showValueColumn}
                    onUpdate={onPointUpdate}
                    columnWidths={columnWidths}
                    errors={errorsByPointIndex?.get(point.point_index)}
                  />
                ))}
              </TableBody>
            </table>
          </div>
        </CollapsibleContent>
      </Collapsible>
    </div>
  )
}

function formatOffsetName(name: string): string {
  return name
    .split('_')
    .map((word) => word.charAt(0).toUpperCase() + word.slice(1))
    .join(' ')
}

export default OffsetSection
