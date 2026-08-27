import { Button } from '@/components/ui/button'
import { Badge } from '@/components/ui/badge'
import { ScrollArea } from '@/components/ui/scroll-area'
import {
  getScheduleColor,
  formatDateTime,
  Schedule,
} from '../../utils/scheduleUtils'

interface ScheduleListProps {
  schedules: Schedule[]
  selectedIndex: number | null
  onSelect: (index: number) => void
  onAdd: () => void
  onDelete: (index: number) => void
}

function ScheduleList({
  schedules,
  selectedIndex,
  onSelect,
  onAdd,
  onDelete,
}: ScheduleListProps) {
  const configuredSchedules = schedules.filter((s) => s.identity > 0)
  const displaySchedules =
    configuredSchedules.length > 0 ? configuredSchedules : schedules.slice(0, 3)

  return (
    <div className="flex flex-col h-full">
      <div className="flex items-center justify-between p-2 border-b border-border">
        <span className="font-semibold text-sm">Schedules</span>
        <Button size="xs" variant="secondary" onClick={onAdd}>
          + Add Schedule
        </Button>
      </div>

      <ScrollArea className="flex-1 min-h-0 p-1.5">
        <div className="flex flex-col gap-1.5">
          {displaySchedules.map((schedule) => (
            <ScheduleCard
              key={schedule.index}
              schedule={schedule}
              isSelected={selectedIndex === schedule.index}
              onSelect={() => onSelect(schedule.index)}
              onDelete={() => onDelete(schedule.index)}
            />
          ))}

          {displaySchedules.length === 0 && (
            <div className="flex flex-col items-center p-6 gap-1">
              <p className="text-muted-foreground text-sm">
                No schedules configured
              </p>
              <p className="text-muted-foreground text-xs">
                Click &quot;+ Add&quot; to create one
              </p>
            </div>
          )}
        </div>
      </ScrollArea>
    </div>
  )
}

interface ScheduleCardProps {
  schedule: Schedule
  isSelected: boolean
  onSelect: () => void
  onDelete: () => void
}

function ScheduleCard({
  schedule,
  isSelected,
  onSelect,
  onDelete,
}: ScheduleCardProps) {
  const color = getScheduleColor(schedule.index)
  const hasTimeRange = schedule.startDateTime && schedule.stopDateTime

  const handleDelete = (e: React.MouseEvent) => {
    e.stopPropagation()
    if (confirm(`Delete Schedule ${schedule.index}?`)) {
      onDelete()
    }
  }

  const handleKeyDown = (e: React.KeyboardEvent<HTMLDivElement>) => {
    if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault()
      onSelect()
    }
  }

  return (
    <div
      role="button"
      tabIndex={0}
      onClick={onSelect}
      onKeyDown={handleKeyDown}
      className={`w-full text-left cursor-pointer p-2 border rounded-md ${isSelected ? 'bg-primary/5 border-primary/40' : 'hover:bg-muted/50'}`}
      style={{ borderLeft: `3px solid ${color}` }}
    >
      <div className="flex items-center justify-between mb-0.5">
        <span className="font-semibold text-xs">Schedule {schedule.index}</span>
        <div className="flex items-center gap-1">
          <Badge variant="secondary" className="text-[10px] px-1.5 py-0">
            P{schedule.priority}
          </Badge>
          <Badge variant="outline" className="text-[10px] px-1.5 py-0">
            {schedule.numberOfPoints} pts
          </Badge>
        </div>
      </div>

      {hasTimeRange ? (
        <div className="flex flex-col gap-px">
          <span className="text-xs text-muted-foreground font-mono">
            {formatDateTime(schedule.startDateTime)}
          </span>
          <span className="text-xs text-muted-foreground font-mono">
            {formatDateTime(schedule.stopDateTime)}
          </span>
        </div>
      ) : (
        <span className="text-xs text-muted-foreground italic">
          No time range set
        </span>
      )}

      <div className="flex justify-end mt-1">
        <Button
          size="xs"
          variant="ghost"
          className="text-danger hover:text-danger hover:bg-danger/10 h-5 px-1.5 text-[10px]"
          onClick={handleDelete}
        >
          Delete
        </Button>
      </div>
    </div>
  )
}

export default ScheduleList
