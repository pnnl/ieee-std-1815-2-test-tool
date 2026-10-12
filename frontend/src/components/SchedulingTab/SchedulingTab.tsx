import { useState, useEffect, useCallback, useRef } from 'react'
import { createPortal } from 'react-dom'
import ScheduleList from './ScheduleList'
import ScheduleEditor from './ScheduleEditor'
import GanttVisualization from './GanttVisualization'
import {
  extractSchedulesFromProfile,
  createEmptySchedule,
  applySchedulesToProfile,
  Schedule,
  scheduleToEvents,
} from '../../utils/scheduleUtils'
import type { PicsProfile, ValidationError } from '@/api/generated'
import { toast } from 'sonner'
import ValidationErrorCallout from '../ValidationErrorCallout'

interface SchedulingTabProps {
  profileData: PicsProfile
  setProfileData: (data: PicsProfile) => void
  // Schedule-structural errors bucketed to this tab, same treatment as
  // Curves: no point rows or offset groups, so scope is a tab badge plus
  // this panel listing.
  errors: ValidationError[]
}

function SchedulingTab({
  profileData,
  setProfileData,
  errors,
}: SchedulingTabProps) {
  const [schedules, setSchedules] = useState<Schedule[]>([])
  const [selectedIndex, setSelectedIndex] = useState<number | null>(null)
  const [ganttPoppedOut, setGanttPoppedOut] = useState(false)
  const [floatingPos, setFloatingPos] = useState({ x: 100, y: 80 })
  const [floatingSize, setFloatingSize] = useState({ w: 700, h: 420 })
  const dragRef = useRef<{
    startX: number
    startY: number
    origX: number
    origY: number
  } | null>(null)
  const resizeRef = useRef<{
    startX: number
    startY: number
    origW: number
    origH: number
  } | null>(null)

  useEffect(() => {
    if (profileData) {
      const extracted = extractSchedulesFromProfile(profileData)

      setSchedules(extracted)

      if (selectedIndex === null && extracted.length > 0) {
        setSelectedIndex(extracted[0].index)
      }
    }
  }, [profileData, selectedIndex])

  const selectedSchedule =
    schedules.find((s) => s.index === selectedIndex) || null

  const saveSchedulesToProfile = useCallback(
    (newSchedules: Schedule[]) => {
      if (!profileData || !setProfileData) return
      setProfileData(applySchedulesToProfile(profileData, newSchedules))
    },
    [profileData, setProfileData],
  )

  const handleSelectSchedule = useCallback((index: number) => {
    setSelectedIndex(index)
  }, [])

  const handleScheduleChange = useCallback(
    (updatedSchedule: Schedule) => {
      setSchedules((prev) => {
        const newSchedules = [...prev]
        const idx = newSchedules.findIndex(
          (s) => s.index === updatedSchedule.index,
        )
        if (idx !== -1) {
          newSchedules[idx] = updatedSchedule
        } else {
          newSchedules.push(updatedSchedule)
        }
        saveSchedulesToProfile(newSchedules)
        return newSchedules
      })
    },
    [saveSchedulesToProfile],
  )

  const handleAddSchedule = useCallback(() => {
    // Reuse any existing slot whose identity is still the placeholder 0
    // (canonical seeds always ship with one such schedule). If every slot
    // is already configured, grow the array.
    let newSchedules: Schedule[]
    let assignedIndex: number
    const placeholderArrayPos = schedules.findIndex((s) => s.identity === 0)
    if (placeholderArrayPos >= 0) {
      const replacement = createEmptySchedule(placeholderArrayPos)
      newSchedules = [...schedules]
      newSchedules[placeholderArrayPos] = replacement
      assignedIndex = placeholderArrayPos
    } else {
      const usedIndices = new Set(schedules.map((s) => s.index))
      let nextIndex = 0
      while (usedIndices.has(nextIndex) && nextIndex < 100) {
        nextIndex++
      }
      if (nextIndex >= 100) {
        toast.warning('Maximum of 100 schedules reached')
        return
      }
      newSchedules = [...schedules, createEmptySchedule(nextIndex)]
      assignedIndex = nextIndex
    }

    setSchedules(newSchedules)
    setSelectedIndex(assignedIndex)
    saveSchedulesToProfile(newSchedules)

    toast.success(`Schedule ${assignedIndex} created`)
  }, [schedules, saveSchedulesToProfile])

  const handleDeleteSchedule = useCallback(
    (index: number) => {
      const newSchedules = schedules.filter((s) => s.index !== index)

      setSchedules(newSchedules)
      saveSchedulesToProfile(newSchedules)

      if (selectedIndex === index) {
        if (newSchedules.length > 0) {
          setSelectedIndex(newSchedules[0].index)
        } else {
          setSelectedIndex(null)
        }
      }

      toast.success(`Schedule ${index} deleted`)
    },
    [schedules, selectedIndex, saveSchedulesToProfile],
  )

  const handleDragStart = useCallback(
    (e: React.MouseEvent) => {
      e.preventDefault()
      dragRef.current = {
        startX: e.clientX,
        startY: e.clientY,
        origX: floatingPos.x,
        origY: floatingPos.y,
      }

      const onMove = (ev: MouseEvent) => {
        if (!dragRef.current) return
        const dx = ev.clientX - dragRef.current.startX
        const dy = ev.clientY - dragRef.current.startY
        setFloatingPos({
          x: Math.max(0, dragRef.current.origX + dx),
          y: Math.max(0, dragRef.current.origY + dy),
        })
      }
      const onUp = () => {
        dragRef.current = null
        window.removeEventListener('mousemove', onMove)
        window.removeEventListener('mouseup', onUp)
      }
      window.addEventListener('mousemove', onMove)
      window.addEventListener('mouseup', onUp)
    },
    [floatingPos],
  )

  const handleResizeStart = useCallback(
    (edge: 'right' | 'bottom' | 'corner', e: React.MouseEvent) => {
      e.preventDefault()
      e.stopPropagation()
      resizeRef.current = {
        startX: e.clientX,
        startY: e.clientY,
        origW: floatingSize.w,
        origH: floatingSize.h,
      }

      const onMove = (ev: MouseEvent) => {
        if (!resizeRef.current) return
        const dw = ev.clientX - resizeRef.current.startX
        const dh = ev.clientY - resizeRef.current.startY
        setFloatingSize({
          w:
            edge === 'bottom'
              ? resizeRef.current.origW
              : Math.max(400, resizeRef.current.origW + dw),
          h:
            edge === 'right'
              ? resizeRef.current.origH
              : Math.max(250, resizeRef.current.origH + dh),
        })
      }
      const onUp = () => {
        resizeRef.current = null
        window.removeEventListener('mousemove', onMove)
        window.removeEventListener('mouseup', onUp)
      }
      window.addEventListener('mousemove', onMove)
      window.addEventListener('mouseup', onUp)
    },
    [floatingSize],
  )

  if (!profileData) {
    return (
      <div className="flex flex-col items-center justify-center h-[300px] text-muted-foreground">
        <h3 className="text-lg font-semibold">No Profile Loaded</h3>
        <p className="text-sm">Load a profile to configure schedules.</p>
      </div>
    )
  }

  const ganttContent = (
    <GanttVisualization
      events={schedules.flatMap(scheduleToEvents)}
      selectedIndex={selectedIndex}
      onSelectSchedule={handleSelectSchedule}
    />
  )

  const floatingPanel = ganttPoppedOut
    ? createPortal(
        <div
          className="fixed border border-border rounded-lg shadow-2xl bg-background flex flex-col overflow-hidden"
          style={{
            left: floatingPos.x,
            top: floatingPos.y,
            width: floatingSize.w,
            height: floatingSize.h,
            zIndex: 9999,
          }}
        >
          <div
            className="flex items-center justify-between px-3 py-1.5 bg-muted border-b border-border cursor-move select-none"
            onMouseDown={handleDragStart}
          >
            <span className="text-xs font-semibold text-muted-foreground">
              Gantt Visualization
            </span>
            <button
              onClick={() => setGanttPoppedOut(false)}
              className="text-xs px-2 py-0.5 rounded hover:bg-background text-muted-foreground cursor-pointer"
            >
              Dock
            </button>
          </div>
          <div className="flex-1 min-h-0 overflow-auto">{ganttContent}</div>
          {/* Right edge */}
          <div
            className="absolute top-0 right-0 w-2 h-full cursor-ew-resize hover:bg-primary/10"
            onMouseDown={(e) => handleResizeStart('right', e)}
          />
          {/* Bottom edge */}
          <div
            className="absolute bottom-0 left-0 w-full h-2 cursor-ns-resize hover:bg-primary/10"
            onMouseDown={(e) => handleResizeStart('bottom', e)}
          />
          {/* Corner grip */}
          <div
            className="absolute bottom-0 right-0 w-4 h-4 cursor-se-resize"
            onMouseDown={(e) => handleResizeStart('corner', e)}
          >
            <svg
              width="16"
              height="16"
              viewBox="0 0 16 16"
              className="text-muted-foreground/50"
            >
              <path
                d="M14 16L16 14M10 16L16 10M6 16L16 6"
                stroke="currentColor"
                strokeWidth="1.5"
                fill="none"
              />
            </svg>
          </div>
        </div>,
        document.body,
      )
    : null

  return (
    <>
      {floatingPanel}
      {errors.length > 0 && (
        <div className="px-4 pt-4">
          <ValidationErrorCallout errors={errors} />
        </div>
      )}
      <div
        className="grid min-h-[500px]"
        style={{
          gridTemplateColumns: '220px 1fr',
          height: 'calc(100vh - 160px)',
        }}
      >
        <div className="overflow-hidden flex flex-col bg-muted">
          <ScheduleList
            schedules={schedules}
            selectedIndex={selectedIndex}
            onSelect={handleSelectSchedule}
            onAdd={handleAddSchedule}
            onDelete={handleDeleteSchedule}
          />
        </div>
        <div className="border-r border-border flex flex-col">
          {!ganttPoppedOut && (
            <div className="overflow-hidden flex flex-col relative">
              {ganttContent}
              <button
                onClick={() => setGanttPoppedOut(true)}
                className="absolute top-2 right-2 text-[10px] px-2 py-0.5 rounded bg-muted border border-border hover:bg-background text-muted-foreground z-10 cursor-pointer"
                title="Pop out Gantt chart"
              >
                Pop Out
              </button>
            </div>
          )}
          <div className="border-t border-border flex flex-col flex-1">
            {ganttPoppedOut && (
              <div className="flex items-center justify-end px-2 py-1 border-b border-border bg-muted/30">
                <button
                  onClick={() => setGanttPoppedOut(false)}
                  className="text-[10px] px-2 py-0.5 rounded border border-dashed border-border text-muted-foreground hover:text-foreground cursor-pointer"
                >
                  Dock Gantt
                </button>
              </div>
            )}
            <ScheduleEditor
              schedule={selectedSchedule}
              onChange={handleScheduleChange}
              profileData={profileData}
            />
          </div>
        </div>
      </div>
    </>
  )
}

export default SchedulingTab
