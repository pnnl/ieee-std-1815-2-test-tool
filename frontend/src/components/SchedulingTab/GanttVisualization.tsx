import { useMemo, useRef } from 'react'
import { GanttEvent, getScheduleColor } from '../../utils/scheduleUtils'

interface GanttVisualizationProps {
  events: GanttEvent[]
  selectedIndex: number | null
  onSelectSchedule: (index: GanttEvent['id']) => void
}

function GanttVisualization({
  events,
  selectedIndex,
  onSelectSchedule: onSelectEvent,
}: GanttVisualizationProps) {
  const containerRef = useRef<HTMLDivElement>(null)

  const timeRange = useMemo(() => {
    if (events.length === 0) {
      const now = new Date()
      const start = new Date(now.getFullYear(), now.getMonth(), now.getDate())
      const end = new Date(start.getTime() + 24 * 60 * 60 * 1000)
      return { start, end }
    }

    let minStart = events[0].startDateTime
    let maxEnd = events[0].stopDateTime

    events.forEach((event) => {
      if (event.startDateTime < minStart) minStart = event.startDateTime
      if (event.stopDateTime > maxEnd) maxEnd = event.stopDateTime
    })

    const duration = maxEnd.getTime() - minStart.getTime()
    const padding = Math.max(duration * 0.05, 30 * 60 * 1000)
    return {
      start: new Date(minStart.getTime() - padding),
      end: new Date(maxEnd.getTime() + padding),
    }
  }, [events])

  // const controllingSegments = useMemo((): ControllingSegment[] => {
  //   if (events.length === 0) return []

  //   const segments: ControllingSegment[] = []
  //   const { start, end } = timeRange
  //   const totalMs = end.getTime() - start.getTime()
  //   const intervalMs = totalMs / 200

  //   let currentController: Schedule | null = null
  //   let segmentStart = start.getTime()

  //   for (let t = start.getTime(); t <= end.getTime(); t += intervalMs) {
  //     const activeSchedules = events.filter(event =>
  //       t >= event.startDateTime.getTime() && t <= event.stopDateTime.getTime()
  //     )

  //     const controller = activeSchedules.length > 0
  //       ? activeSchedules.reduce((a, b) => a.priority < b.priority ? a : b)
  //       : null

  //     const controllerId = controller ? controller.index : null
  //     const currentId = currentController ? currentController.index : null

  //     if (controllerId !== currentId) {
  //       if (currentController !== null) {
  //         segments.push({ schedule: currentController, start: segmentStart, end: t })
  //       }
  //       currentController = controller
  //       segmentStart = t
  //     }
  //   }

  //   if (currentController !== null) {
  //     segments.push({ schedule: currentController, start: segmentStart, end: end.getTime() })
  //   }

  //   return segments
  // }, [validSchedules, timeRange])

  const timeToPercent = (time: number | Date): number => {
    const { start, end } = timeRange
    const totalMs = end.getTime() - start.getTime()
    const offsetMs =
      (typeof time === 'number' ? time : time.getTime()) - start.getTime()
    return (offsetMs / totalMs) * 100
  }

  if (events.length === 0) {
    return (
      <div
        ref={containerRef}
        className="flex flex-col items-center justify-center h-full p-6 bg-background border-l border-border"
      >
        <p className="text-muted-foreground text-sm">
          No schedules with valid time ranges to display.
        </p>
        <p className="text-muted-foreground text-sm">
          Configure start and stop times in the editor.
        </p>
      </div>
    )
  }

  const BAR_HEIGHT = 32
  const STACK_HEIGHT = Math.max(
    4 * BAR_HEIGHT + 4,
    events.length * BAR_HEIGHT + 4,
  )
  // const CONTROL_BAR_HEIGHT = 36

  return (
    <div
      ref={containerRef}
      className="flex flex-col gap-2 p-4 bg-background border-l border-border overflow-auto"
    >
      {/* Stacked Schedule Bars */}
      <div className="flex border border-border bg-muted/50">
        <div
          className="flex-1 relative bg-background overflow-hidden"
          style={{ height: STACK_HEIGHT }}
        >
          {/* Time axis line */}
          <div className="absolute bottom-0 left-0 right-0 h-0.5 bg-[#1a5276]" />

          {/* Schedule bars */}
          {events.map((event, idx) => {
            const left = timeToPercent(event.startDateTime)
            const right = timeToPercent(event.stopDateTime)
            const width = right - left
            const bottom = idx * BAR_HEIGHT
            const schedColor = getScheduleColor(event.groupId)
            const isSelected = selectedIndex === event.id

            return (
              <div
                key={event.id}
                title={`${event.label}`}
                onClick={() => onSelectEvent(event.id)}
                className="absolute flex items-center justify-center flex-col cursor-pointer"
                style={{
                  left: `${left}%`,
                  width: `${width}%`,
                  bottom,
                  height: BAR_HEIGHT - 2,
                  backgroundColor: schedColor,
                  zIndex: events.length - idx,
                  border: isSelected
                    ? '2px solid #333'
                    : '1px solid rgba(0,0,0,0.15)',
                  boxSizing: 'border-box',
                  minWidth: 60,
                }}
              >
                <span className="text-xs font-semibold text-gray-900 whitespace-nowrap overflow-hidden text-ellipsis max-w-[90%]">
                  {event.label}
                </span>
                <span className="text-[9px] text-gray-600 whitespace-nowrap">
                  Priority {event.priority}
                </span>
              </div>
            )
          })}
        </div>
      </div>

      {/* <div className="min-w-[100px] bg-muted border-r border-border flex flex-col justify-center">
        <span className="text-xs italic text-muted-foreground leading-snug">Resolved schedule</span>
      </div> */}

      {/* Controlling Schedule Output Track
      <div className="flex border border-border bg-muted/50">
        <div className="flex-1 relative h-[50px] bg-[#e8e8e8]">
          {controllingSegments.map((segment, i) => {
            const left = timeToPercent(segment.start)
            const right = timeToPercent(segment.end)
            const width = right - left
            const schedColor = getScheduleColor(segment.schedule.index)
            const label = String(segment.schedule.index)

            return (
              <div
                key={i}
                title={`Schedule ${label}`}
                className="absolute flex items-center justify-center"
                style={{
                  left: `${left}%`,
                  width: `${width}%`,
                  top: 7,
                  height: CONTROL_BAR_HEIGHT,
                  backgroundColor: schedColor,
                  border: '1px solid rgba(0,0,0,0.1)',
                  boxSizing: 'border-box',
                }}
              >
                {width > 8 && (
                  <span className="text-[10px] font-medium text-gray-900 whitespace-nowrap overflow-hidden text-ellipsis">
                    Schedule {label}
                  </span>
                )}
              </div>
            )
          })}
        </div>
      </div> */}

      {/* Time Label */}
      <div className="flex items-center justify-end gap-1 px-1">
        <span className="text-xs italic text-muted-foreground">Time</span>
        <span className="text-base text-primary">&rarr;</span>
      </div>
    </div>
  )
}

export default GanttVisualization
