import { useState } from 'react'
import { Button } from '@/components/ui/button'
import { NumberInput } from '@/components/ui/number-input'
import { NativeSelect } from '@/components/ui/native-select'
import {
  Table,
  TableHeader,
  TableBody,
  TableRow,
  TableHead,
  TableCell,
} from '@/components/ui/table'
import { ACTION_TYPES, SchedulePoint } from '../../utils/scheduleUtils'

interface SchedulePointEditorProps {
  points: SchedulePoint[]
  onChange: (points: SchedulePoint[]) => void
}

function SchedulePointEditor({ points, onChange }: SchedulePointEditorProps) {
  const [isAdding, setIsAdding] = useState(false)
  const [editingIndex, setEditingIndex] = useState<number | null>(null)
  const [newPoint, setNewPoint] = useState<SchedulePoint>({
    timeOffset: 0,
    actionType: 1,
    actionIndex: 0,
    value: 0,
  })

  const actionTypeOptions = Object.entries(ACTION_TYPES).map(
    ([value, label]) => ({
      value,
      label,
    }),
  )

  const handleAdd = () => {
    if (points.length >= 100) {
      alert('Maximum of 100 points per schedule')
      return
    }
    const updatedPoints = [...points, { ...newPoint }]
    onChange(updatedPoints)
    setNewPoint({ timeOffset: 0, actionType: 1, actionIndex: 0, value: 0 })
    setIsAdding(false)
  }

  const handleUpdate = (
    index: number,
    field: keyof SchedulePoint,
    value: number,
  ) => {
    const updatedPoints = [...points]
    updatedPoints[index] = { ...updatedPoints[index], [field]: value }
    onChange(updatedPoints)
  }

  const handleDelete = (index: number) => {
    if (confirm('Delete this action point?')) {
      const updatedPoints = points.filter((_, i) => i !== index)
      onChange(updatedPoints)
    }
  }

  const handleMoveUp = (index: number) => {
    if (index === 0) return
    const updatedPoints = [...points]
    const temp = updatedPoints[index]
    updatedPoints[index] = updatedPoints[index - 1]
    updatedPoints[index - 1] = temp
    onChange(updatedPoints)
  }

  const handleMoveDown = (index: number) => {
    if (index === points.length - 1) return
    const updatedPoints = [...points]
    const temp = updatedPoints[index]
    updatedPoints[index] = updatedPoints[index + 1]
    updatedPoints[index + 1] = temp
    onChange(updatedPoints)
  }

  const formatTimeOffset = (seconds: number): string => {
    if (seconds < 60) return `${seconds}s`
    if (seconds < 3600) return `${Math.floor(seconds / 60)}m ${seconds % 60}s`
    const hours = Math.floor(seconds / 3600)
    const mins = Math.floor((seconds % 3600) / 60)
    return `${hours}h ${mins}m`
  }

  const getActionTypeName = (type: number): string =>
    ACTION_TYPES[type] || `Type ${type}`

  return (
    <>
      <Table>
        <TableHeader>
          <TableRow>
            <TableHead className="w-10">#</TableHead>
            <TableHead>Time Offset</TableHead>
            <TableHead>Action Type</TableHead>
            <TableHead>Target Point</TableHead>
            <TableHead>Value</TableHead>
            <TableHead className="w-[160px]">Actions</TableHead>
          </TableRow>
        </TableHeader>
        <TableBody>
          {points.map((point, index) => (
            <TableRow
              key={index}
              className={editingIndex === index ? 'bg-primary/5' : ''}
            >
              <TableCell className="text-xs">{index + 1}</TableCell>
              <TableCell>
                {editingIndex === index ? (
                  <NumberInput
                    size="xs"
                    value={point.timeOffset}
                    onChange={(val) =>
                      handleUpdate(index, 'timeOffset', Number(val) || 0)
                    }
                    min={0}
                    className="w-20"
                  />
                ) : (
                  <span
                    className="text-xs"
                    title={`${point.timeOffset} seconds`}
                  >
                    {formatTimeOffset(point.timeOffset)}
                  </span>
                )}
              </TableCell>
              <TableCell>
                {editingIndex === index ? (
                  <NativeSelect
                    size="sm"
                    value={String(point.actionType)}
                    onChange={(e) =>
                      handleUpdate(
                        index,
                        'actionType',
                        parseInt(e.currentTarget.value),
                      )
                    }
                    data={actionTypeOptions}
                  />
                ) : (
                  <span className="text-xs">
                    {getActionTypeName(point.actionType)}
                  </span>
                )}
              </TableCell>
              <TableCell>
                {editingIndex === index ? null : (
                  <span className="text-xs">{point.actionIndex}</span>
                )}
              </TableCell>
              <TableCell>
                {editingIndex === index ? (
                  <NumberInput
                    size="xs"
                    value={point.value}
                    onChange={(val) =>
                      handleUpdate(index, 'value', Number(val) || 0)
                    }
                    className="w-20"
                  />
                ) : (
                  <span className="text-xs">{point.value}</span>
                )}
              </TableCell>
              <TableCell>
                <div className="flex items-center gap-1 flex-nowrap">
                  {editingIndex === index ? (
                    <Button
                      size="xs"
                      variant="outline"
                      className="text-success border-success"
                      onClick={() => setEditingIndex(null)}
                    >
                      Done
                    </Button>
                  ) : (
                    <>
                      <Button
                        size="xs"
                        variant="outline"
                        onClick={() => setEditingIndex(index)}
                      >
                        Edit
                      </Button>
                      <Button
                        size="xs"
                        variant="outline"
                        onClick={() => handleMoveUp(index)}
                        disabled={index === 0}
                      >
                        Up
                      </Button>
                      <Button
                        size="xs"
                        variant="outline"
                        onClick={() => handleMoveDown(index)}
                        disabled={index === points.length - 1}
                      >
                        Dn
                      </Button>
                      <Button
                        size="xs"
                        variant="outline"
                        className="text-danger border-danger hover:bg-danger/10"
                        onClick={() => handleDelete(index)}
                      >
                        Del
                      </Button>
                    </>
                  )}
                </div>
              </TableCell>
            </TableRow>
          ))}

          {points.length === 0 && !isAdding && (
            <TableRow>
              <TableCell
                colSpan={6}
                className="text-center text-muted-foreground py-6 italic"
              >
                No action points defined. Click &quot;Add Point&quot; to create
                one.
              </TableCell>
            </TableRow>
          )}

          {isAdding && (
            <TableRow className="bg-success/5">
              <TableCell className="text-xs">New</TableCell>
              <TableCell>
                <NumberInput
                  size="xs"
                  value={newPoint.timeOffset}
                  onChange={(val) =>
                    setNewPoint({ ...newPoint, timeOffset: Number(val) || 0 })
                  }
                  min={0}
                  className="w-20"
                />
              </TableCell>
              <TableCell>
                <NativeSelect
                  size="sm"
                  value={String(newPoint.actionType)}
                  onChange={(e) =>
                    setNewPoint({
                      ...newPoint,
                      actionType: parseInt(e.currentTarget.value),
                    })
                  }
                  data={actionTypeOptions}
                />
              </TableCell>
              <TableCell>{null}</TableCell>
              <TableCell>
                <NumberInput
                  size="xs"
                  value={newPoint.value}
                  onChange={(val) =>
                    setNewPoint({ ...newPoint, value: Number(val) || 0 })
                  }
                  className="w-20"
                />
              </TableCell>
              <TableCell>
                <div className="flex items-center gap-1">
                  <Button
                    size="xs"
                    variant="outline"
                    className="text-success border-success"
                    onClick={handleAdd}
                  >
                    Add
                  </Button>
                  <Button
                    size="xs"
                    variant="outline"
                    onClick={() => setIsAdding(false)}
                  >
                    Cancel
                  </Button>
                </div>
              </TableCell>
            </TableRow>
          )}
        </TableBody>
      </Table>

      <div className="flex items-center justify-between mt-2 pt-2 border-t border-border">
        <Button
          variant="outline"
          size="xs"
          onClick={() => setIsAdding(true)}
          disabled={isAdding || points.length >= 100}
        >
          + Add Point
        </Button>
        <span className="text-xs text-muted-foreground">
          {points.length} / 100 points
        </span>
      </div>
    </>
  )
}

export default SchedulePointEditor
