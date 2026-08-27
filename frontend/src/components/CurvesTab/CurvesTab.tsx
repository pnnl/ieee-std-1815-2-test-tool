import { useMemo, useState } from 'react'
import { Button } from '@/components/ui/button'
import { NumberInput } from '@/components/ui/number-input'
import { NativeSelect } from '@/components/ui/native-select'
import { Badge } from '@/components/ui/badge'
import { ScrollArea } from '@/components/ui/scroll-area'
import {
  Table,
  TableHeader,
  TableBody,
  TableRow,
  TableHead,
  TableCell,
} from '@/components/ui/table'
import {
  CURVE_X_UNITS,
  CURVE_Y_UNITS,
  MAX_CURVES,
  MAX_CURVE_POINTS,
  type Curve,
  type CurvePoint,
} from '../../utils/curveUtils'
import { toast } from 'sonner'
import CurveChart from './CurveChart'
import {
  addCurve,
  removeCurve,
  updateCurveHeader,
  updateCurvePointValue,
} from '@/profile/canonical'
import type {
  AiCurve,
  CurveType,
  CurveTypeEntry,
  EnumsResponse,
  PicsProfile,
  ValidationError,
} from '@/api/generated'
import { useEnumData } from '@/contexts/useEnumData'
import ValidationErrorCallout from '../ValidationErrorCallout'

interface CurvesTabProps {
  profileData: PicsProfile
  setProfileData: (data: PicsProfile) => void
  // Curve-structural errors bucketed to this tab. No point rows or offset
  // groups here like PointsTab, so no group-badge or row-highlight
  // treatment: a tab badge plus this panel listing is the full scope.
  errors: ValidationError[]
}

// Returns `null`, never throws, on no match: a structurally invalid
// curve_type must surface as a rendered error, not crash the tree.
function curveValueToEntry(
  enumResponse: EnumsResponse,
  value: number,
): CurveTypeEntry | null {
  return enumResponse.curve_types.find((e) => e.std_number === value) ?? null
}

function curveTypeToValue(
  enumResponse: EnumsResponse,
  curveType: CurveType,
): number {
  const entry = enumResponse.curve_types.find((e) => e.variant === curveType)

  if (!entry) {
    throw new Error(`Unknown curve type variant: ${curveType}`)
  }

  return entry.std_number
}

function constructCurveView(
  enumResponse: EnumsResponse,
  curve: AiCurve,
): Curve {
  const xs = curve.x_values
  const ys = curve.y_values
  const len = Math.min(xs.length, ys.length)
  const points: CurvePoint[] = []
  const curveEntry = curveValueToEntry(enumResponse, curve.curve_type.value)
  for (let i = 0; i < len; i++) {
    points.push({ x: xs[i].value, y: ys[i].value })
  }
  return {
    curveTypeEntry: curveEntry,
    curveTypeRawValue: curve.curve_type.value,
    number_of_points: curve.number_of_points.value,
    x_units: curve.x_units.value as keyof typeof CURVE_X_UNITS,
    y_units: curve.y_units.value as keyof typeof CURVE_Y_UNITS,
    points,
  }
}

function CurvesTab({ profileData, setProfileData, errors }: CurvesTabProps) {
  const { enumsResponse } = useEnumData()

  const curves = useMemo(() => {
    return profileData.AI.curves.map(
      constructCurveView.bind(null, enumsResponse),
    )
  }, [enumsResponse, profileData.AI.curves])
  const [selectedCurveIndex, setSelectedCurveIndex] = useState<number | null>(
    curves.length > 0 ? 0 : null,
  )

  const currentCurveCount = curves.length

  const handleAddCurve = () => {
    if (currentCurveCount >= MAX_CURVES) {
      toast.error(`A profile can contain at most ${MAX_CURVES} curves`)
      return
    }
    if (profileData.AI.curves.length === 0) {
      toast.error(
        'No template curve to clone: load a profile with at least one curve first.',
      )
      return
    }
    const next = addCurve(profileData)
    setProfileData(next)
    setSelectedCurveIndex(next.AI.curves.length - 1)
  }

  const handleDeleteCurve = () => {
    if (selectedCurveIndex === null) {
      toast.error('No curve selected to delete')
      return
    }
    const next = removeCurve(profileData, selectedCurveIndex)
    setProfileData(next)
    setSelectedCurveIndex(
      next.AI.curves.length === 0
        ? null
        : Math.min(selectedCurveIndex, next.AI.curves.length - 1),
    )
  }

  if (selectedCurveIndex === null || currentCurveCount === 0) {
    return (
      <div className="flex flex-col gap-4 p-4">
        <ValidationErrorCallout errors={errors} />
        <div className="flex flex-col items-center justify-center h-full gap-4">
          <h3 className="text-lg font-semibold">No curves configured</h3>
          <p className="text-sm text-muted-foreground">
            Add a curve here or set the curve count in the Entities tab.
          </p>
          <Button size="sm" onClick={handleAddCurve}>
            Add Curve
          </Button>
        </div>
      </div>
    )
  }

  const curveConfig = curves[selectedCurveIndex]
  const curvePoints = curveConfig.points

  const handleCurveTypeChange = (value: CurveType) => {
    setProfileData(
      updateCurveHeader(
        profileData,
        selectedCurveIndex,
        'curve_type',
        curveTypeToValue(enumsResponse, value),
      ),
    )
  }

  const handleConfigChange = (
    field: 'number_of_points' | 'x_units' | 'y_units',
    value: string,
  ) => {
    const numValue = parseInt(value)
    if (Number.isNaN(numValue)) {
      toast.error(`Invalid value for '${field}': '${value}'`)
      return
    }
    setProfileData(
      updateCurveHeader(profileData, selectedCurveIndex, field, numValue),
    )
  }

  const handlePointChange = (
    pointIdx: number,
    axis: 'x' | 'y',
    value: string,
  ) => {
    const numValue = parseInt(value)
    if (Number.isNaN(numValue)) {
      toast.error(`Invalid value for point ${pointIdx} ${axis}-axis: ${value}`)
      return
    }
    setProfileData(
      updateCurvePointValue(
        profileData,
        selectedCurveIndex,
        axis,
        pointIdx,
        numValue,
      ),
    )
  }

  const activePointCount = curveConfig.number_of_points || 0
  const activePoints = curvePoints.slice(
    0,
    Math.min(activePointCount, MAX_CURVE_POINTS),
  )

  const curveSelectData = Array.from(
    { length: Math.max(1, currentCurveCount) },
    (_, i) => ({
      value: String(i),
      label: `Curve ${i + 1}`,
    }),
  )

  const curveTypeData =
    enumsResponse.curve_types.map(({ variant, display_name }) => ({
      value: variant,
      label: display_name,
    })) ?? []
  const xUnitsData = Object.entries(CURVE_X_UNITS).map(([value, label]) => ({
    value,
    label,
  }))
  const yUnitsData = Object.entries(CURVE_Y_UNITS).map(([value, label]) => ({
    value,
    label,
  }))

  return (
    <>
      {errors.length > 0 && (
        <div className="px-4 pt-4">
          <ValidationErrorCallout errors={errors} />
        </div>
      )}
      <div
        className="grid min-h-[500px]"
        style={{
          gridTemplateColumns: '340px 1fr',
          height: 'calc(100vh - 160px)',
        }}
      >
        <div className="border-r border-border flex flex-col overflow-hidden">
          <div className="flex items-center justify-between p-2 border-b border-border bg-muted">
            <h4 className="text-base font-semibold">Curve Editor</h4>
          </div>

          <ScrollArea className="flex-1 min-h-0 p-3">
            <div className="flex flex-col gap-4">
              <div className="flex items-end gap-2">
                <NativeSelect
                  className="flex-1"
                  label="Select Curve"
                  value={String(selectedCurveIndex)}
                  onChange={(e) =>
                    setSelectedCurveIndex(parseInt(e.currentTarget.value, 10))
                  }
                  data={curveSelectData}
                  size="sm"
                />
                <Button
                  size="sm"
                  variant="outline"
                  onClick={handleAddCurve}
                  disabled={currentCurveCount >= MAX_CURVES}
                >
                  Add
                </Button>
                <Button size="sm" variant="outline" onClick={handleDeleteCurve}>
                  Delete
                </Button>
              </div>

              <div className="border rounded-sm p-3">
                <p className="font-semibold text-sm mb-2">Configuration</p>
                <div className="flex flex-col gap-2">
                  <NativeSelect
                    label="Type"
                    value={curveConfig.curveTypeEntry?.variant ?? ''}
                    onChange={(e) => {
                      // The "Unrecognized code" placeholder carries value
                      // "": selecting it (the current, already-selected
                      // state) must not feed an empty string into the
                      // typed CurveType handler.
                      const next = e.currentTarget.value
                      if (next === '') return
                      handleCurveTypeChange(next as CurveType)
                    }}
                    data={
                      curveConfig.curveTypeEntry
                        ? curveTypeData
                        : [
                            {
                              value: '',
                              label: `Unrecognized code ${curveConfig.curveTypeRawValue}`,
                            },
                            ...curveTypeData,
                          ]
                    }
                    size="sm"
                  />
                  {!curveConfig.curveTypeEntry && (
                    <p className="text-xs text-destructive">
                      Invalid curve type code {curveConfig.curveTypeRawValue}:
                      not a recognized standard value (0-16). Select a valid
                      type to fix this curve.
                    </p>
                  )}
                  <NumberInput
                    label="Number of Points"
                    value={curveConfig.number_of_points}
                    onChange={(val) =>
                      handleConfigChange('number_of_points', String(val))
                    }
                    min={0}
                    max={MAX_CURVE_POINTS}
                    size="xs"
                  />
                  <div className="grid grid-cols-2 gap-2">
                    <NativeSelect
                      label="X-Axis Units"
                      value={String(curveConfig.x_units)}
                      onChange={(e) =>
                        handleConfigChange('x_units', e.currentTarget.value)
                      }
                      data={xUnitsData}
                      size="sm"
                    />
                    <NativeSelect
                      label="Y-Axis Units"
                      value={String(curveConfig.y_units)}
                      onChange={(e) =>
                        handleConfigChange('y_units', e.currentTarget.value)
                      }
                      data={yUnitsData}
                      size="sm"
                    />
                  </div>
                </div>
              </div>

              <div className="border rounded-sm p-3 flex flex-col min-h-0">
                <div className="flex items-center justify-between mb-2">
                  <p className="font-semibold text-sm">Point Values</p>
                  <span className="text-xs text-muted-foreground">
                    {curvePoints.length} active
                  </span>
                </div>
                <div className="max-h-[45vh] overflow-auto">
                  <Table>
                    <TableHeader>
                      <TableRow>
                        <TableHead className="w-8 text-center">#</TableHead>
                        <TableHead>
                          X ({CURVE_X_UNITS[curveConfig.x_units]})
                        </TableHead>
                        <TableHead>
                          Y ({CURVE_Y_UNITS[curveConfig.y_units]})
                        </TableHead>
                      </TableRow>
                    </TableHeader>
                    <TableBody>
                      {curvePoints.map((point, index) => (
                        <TableRow key={index} className={'bg-primary/5'}>
                          <TableCell className={`text-center text-xs`}>
                            {index}
                          </TableCell>
                          <TableCell>
                            <NumberInput
                              size="xs"
                              step={1}
                              value={point.x}
                              onChange={(val) =>
                                handlePointChange(index, 'x', String(val))
                              }
                              styles={{ input: { fontFamily: 'monospace' } }}
                            />
                          </TableCell>
                          <TableCell>
                            <NumberInput
                              size="xs"
                              step={1}
                              value={point.y}
                              onChange={(val) =>
                                handlePointChange(index, 'y', String(val))
                              }
                              styles={{ input: { fontFamily: 'monospace' } }}
                            />
                          </TableCell>
                        </TableRow>
                      ))}
                    </TableBody>
                  </Table>
                </div>
              </div>
            </div>
          </ScrollArea>
        </div>

        <div className="flex flex-col overflow-hidden">
          <div className="flex items-center justify-center gap-2 p-2 border-b border-border bg-muted">
            <h4 className="text-base font-semibold">
              {curveConfig.curveTypeEntry ? (
                curveConfig.curveTypeEntry.display_name || (
                  <i>curve type not set</i>
                )
              ) : (
                <span className="text-destructive">
                  Invalid curve type ({curveConfig.curveTypeRawValue})
                </span>
              )}
            </h4>
            <Badge variant="outline">Curve {selectedCurveIndex + 1}</Badge>
          </div>

          <div className="flex flex-col items-center justify-center flex-1 p-4">
            <CurveChart
              points={activePoints}
              xLabel={CURVE_X_UNITS[curveConfig.x_units]}
              yLabel={CURVE_Y_UNITS[curveConfig.y_units]}
              onPointMove={(pointIdx, x, y) => {
                let next = updateCurvePointValue(
                  profileData,
                  selectedCurveIndex,
                  'x',
                  pointIdx,
                  x,
                )
                next = updateCurvePointValue(
                  next,
                  selectedCurveIndex,
                  'y',
                  pointIdx,
                  y,
                )
                setProfileData(next)
              }}
            />
            <p className="text-xs text-muted-foreground mt-2">
              Drag points on the chart to adjust values
            </p>
          </div>
        </div>
      </div>
    </>
  )
}

export default CurvesTab
