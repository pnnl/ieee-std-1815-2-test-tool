import { Input } from '@/components/ui/input'
import { TableRow, TableCell } from '@/components/ui/table'
import type { FlatPoint, FlatAiPoint } from '@/profile/canonical'
import type { ValidationError } from '@/api/generated'
import { CheckIcon } from '@radix-ui/react-icons'

type EditableField =
  | 'name'
  | 'mandatory_1815'
  | 'mandatory_1547'
  | 'value'
  | 'purpose'
  | 'units'
  | 'iec_61850_uid'

interface PointRowProps {
  point: FlatPoint
  isBinary: boolean
  showValueColumn: boolean
  onUpdate: (
    point: FlatPoint,
    field: EditableField,
    value: string | number | boolean | null,
  ) => void
  columnWidths?: number[]
  // Errors resolving to this exact point: row-level highlight, one level
  // down from the group badge. Drives a single boolean, not a per-error
  // style, so multiple errors never double the highlight.
  errors?: readonly ValidationError[]
}

function PointRow({
  point,
  isBinary,
  showValueColumn,
  onUpdate,
  columnWidths = [],
  errors,
}: PointRowProps) {
  // Canonical points carry mandatory_1815 (IEEE 1815.2 mandatory) and
  // mandatory_1547 (IEEE 1547.1 mandatory). The legacy "supported" field
  // doesn't exist; if a profile dropped a point, it'd be absent from the
  // canonical structure entirely. We therefore drop the supported checkbox.
  const associated = associatedNameOf(point) ?? ''
  const units = unitsOf(point) ?? ''
  const valueDisplay = valueOf(point)
  const hasError = !!errors && errors.length > 0
  // Non-color affordance: `title` (hover tooltip) and `aria-describedby`
  // pointing at a `sr-only` span, rather than `aria-label`, which would
  // replace the row's whole accessible name with just the error text.
  const errorSummary = hasError
    ? errors!.map((e) => e.message).join('; ')
    : undefined
  const errorDescriptionId = hasError
    ? `point-error-${point.kind}-${point.point_index}`
    : undefined

  return (
    <TableRow
      className={
        hasError
          ? // Naming our own hover:bg-destructive/* lets twMerge dedupe the
            // conflicting hover:bg-* utility in our favor, so the error
            // stays red under the cursor instead of flashing to hover gray.
            'bg-destructive/10 hover:bg-destructive/25 dark:hover:bg-destructive/35 border-l-[3px] border-l-destructive'
          : point.mandatory_1815
            ? 'bg-primary/8 border-l-[3px] border-l-primary'
            : 'bg-muted/30 border-l-[3px] border-l-muted-foreground/40'
      }
      title={errorSummary}
      aria-describedby={errorDescriptionId}
    >
      <TableCell
        style={{ width: columnWidths[0] ? `${columnWidths[0]}px` : undefined }}
      >
        {prefixOf(point)}
        {point.point_index}
        {hasError && (
          <span id={errorDescriptionId} className="sr-only">
            {errorSummary}
          </span>
        )}
      </TableCell>

      <TableCell
        className="whitespace-normal break-words"
        style={{ width: columnWidths[1] ? `${columnWidths[1]}px` : undefined }}
      >
        {point.name || ''}
      </TableCell>

      <TableCell
        className="whitespace-normal break-words"
        style={{ width: columnWidths[2] ? `${columnWidths[2]}px` : undefined }}
      >
        {point.iec_61850_uid || ''}
      </TableCell>

      <TableCell
        className="whitespace-normal break-words"
        style={{ width: columnWidths[3] ? `${columnWidths[3]}px` : undefined }}
      >
        {associated}
      </TableCell>

      <TableCell
        className="whitespace-normal break-words"
        style={{ width: columnWidths[4] ? `${columnWidths[4]}px` : undefined }}
      >
        {point.purpose || ''}
      </TableCell>

      {/* Mandatory 1547 */}
      <TableCell
        className="text-center"
        style={{ width: columnWidths[5] ? `${columnWidths[5]}px` : undefined }}
      >
        {point.mandatory_1547 && <CheckIcon></CheckIcon>}
      </TableCell>

      {/* Mandatory 1815 (IEEE 1815.2) */}
      <TableCell
        className="text-center"
        style={{ width: columnWidths[6] ? `${columnWidths[6]}px` : undefined }}
      >
        {point.mandatory_1815 && <CheckIcon></CheckIcon>}
      </TableCell>

      {/* Units (analog only) */}
      {!isBinary && (
        <TableCell
          className="whitespace-normal break-words"
          style={{
            width: columnWidths[7] ? `${columnWidths[7]}px` : undefined,
          }}
        >
          {units}
        </TableCell>
      )}

      {/* Value column: AI only */}
      {showValueColumn && (
        <TableCell
          style={{
            width: columnWidths[isBinary ? 7 : 8]
              ? `${columnWidths[isBinary ? 7 : 8]}px`
              : undefined,
          }}
        >
          <Input
            type="number"
            className="h-7 text-xs font-mono"
            value={valueDisplay}
            onChange={(e) => {
              const raw = e.currentTarget.value
              const parsed = raw === '' ? null : Number(raw)
              if (parsed !== null && Number.isNaN(parsed)) return
              onUpdate(point, 'value', parsed)
            }}
          />
        </TableCell>
      )}
    </TableRow>
  )
}

function prefixOf(point: FlatPoint): string {
  switch (point.kind) {
    case 'bo':
      return 'BO'
    case 'bi':
      return 'BI'
    case 'ao':
      return 'AO'
    case 'ai':
      return 'AI'
  }
}

function associatedNameOf(point: FlatPoint): string | null {
  switch (point.kind) {
    case 'bo':
      return point.assoc_bi ?? null
    case 'bi':
      return point.assoc_bo ?? null
    case 'ao':
      return point.assoc_ai ?? null
    case 'ai':
      return point.assoc_ao ?? null
  }
}

function unitsOf(point: FlatPoint): string | null {
  switch (point.kind) {
    case 'bo':
    case 'bi':
      return null
    case 'ao':
    case 'ai':
      return point.units
  }
}

function valueOf(point: FlatPoint): string {
  if (point.kind !== 'ai') return ''
  const ai = point as FlatAiPoint
  if (ai.value === null || ai.value === undefined) return ''
  return String(ai.value)
}

export default PointRow
