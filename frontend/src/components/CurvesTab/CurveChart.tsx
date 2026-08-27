import { CurvePoint } from '@/utils/curveUtils'
import { useMemo, useState, useRef, useCallback } from 'react'

const CHART_WIDTH = 500
const CHART_HEIGHT = 400
const PADDING = { top: 30, right: 30, bottom: 50, left: 60 }
const INNER_WIDTH = CHART_WIDTH - PADDING.left - PADDING.right
const INNER_HEIGHT = CHART_HEIGHT - PADDING.top - PADDING.bottom

interface CurveChartProps {
  points: CurvePoint[]
  xLabel: string
  yLabel: string
  onPointMove?: (pointIndex: number, x: number, y: number) => void
}

function CurveChart({ points, xLabel, yLabel, onPointMove }: CurveChartProps) {
  const svgRef = useRef<SVGSVGElement>(null)
  const [draggingPoint, setDraggingPoint] = useState<number | null>(null)
  const [hoveredPoint, setHoveredPoint] = useState<number | null>(null)

  const bounds = useMemo(() => {
    if (!points || points.length === 0) {
      return { xMin: 0, xMax: 100, yMin: 0, yMax: 100 }
    }

    const xValues = points.map((p) => p.x)
    const yValues = points.map((p) => p.y)

    let xMin = Math.min(...xValues)
    let xMax = Math.max(...xValues)
    let yMin = Math.min(...yValues)
    let yMax = Math.max(...yValues)

    const xPadding = (xMax - xMin) * 0.1 || 10
    const yPadding = (yMax - yMin) * 0.1 || 10

    xMin -= xPadding
    xMax += xPadding
    yMin -= yPadding
    yMax += yPadding

    return { xMin, xMax, yMin, yMax }
  }, [points])

  const scaleX = useCallback(
    (x: number) => {
      const ratio = (x - bounds.xMin) / (bounds.xMax - bounds.xMin)
      return PADDING.left + ratio * INNER_WIDTH
    },
    [bounds],
  )

  const scaleY = useCallback(
    (y: number) => {
      const ratio = (y - bounds.yMin) / (bounds.yMax - bounds.yMin)
      return PADDING.top + INNER_HEIGHT - ratio * INNER_HEIGHT
    },
    [bounds],
  )

  const unscaleX = (px: number) => {
    const ratio = (px - PADDING.left) / INNER_WIDTH
    return bounds.xMin + ratio * (bounds.xMax - bounds.xMin)
  }

  const unscaleY = (py: number) => {
    const ratio = (PADDING.top + INNER_HEIGHT - py) / INNER_HEIGHT
    return bounds.yMin + ratio * (bounds.yMax - bounds.yMin)
  }

  const xTicks = useMemo(() => {
    const min = Math.ceil(bounds.xMin)
    const max = Math.floor(bounds.xMax)
    const range = max - min
    const step = Math.max(1, Math.round(range / 5))
    const ticks: number[] = []
    for (let tickValue = min; tickValue <= max; tickValue += step) {
      ticks.push(tickValue)
    }
    return ticks
  }, [bounds])

  const yTicks = useMemo(() => {
    const min = Math.ceil(bounds.yMin)
    const max = Math.floor(bounds.yMax)
    const range = max - min
    const step = Math.max(1, Math.round(range / 5))
    const ticks: number[] = []
    for (let tickValue = min; tickValue <= max; tickValue += step) {
      ticks.push(tickValue)
    }
    return ticks
  }, [bounds])

  const linePath = useMemo(() => {
    if (!points || points.length === 0) return ''
    const sorted = [...points].sort((a, b) => a.x - b.x)
    return sorted
      .map((p, i) => `${i === 0 ? 'M' : 'L'} ${scaleX(p.x)} ${scaleY(p.y)}`)
      .join(' ')
  }, [points, scaleX, scaleY])

  const handleMouseDown = (e: React.MouseEvent, pointIndex: number) => {
    e.preventDefault()
    setDraggingPoint(pointIndex)
  }

  const handleMouseMove = (e: React.MouseEvent) => {
    if (draggingPoint === null || !svgRef.current) return

    const svgRect = svgRef.current.getBoundingClientRect()
    const px = e.clientX - svgRect.left
    const py = e.clientY - svgRect.top

    const newX = Math.round(unscaleX(px))
    const newY = Math.round(unscaleY(py))

    if (onPointMove) {
      onPointMove(draggingPoint, newX, newY)
    }
  }

  const handleMouseUp = () => {
    setDraggingPoint(null)
  }

  if (!points || points.length === 0) {
    return (
      <div className="flex flex-col items-center justify-center h-[400px] text-muted-foreground">
        <p>No curve points to display.</p>
        <p className="text-sm">
          Set &quot;Number of Points&quot; greater than 0 to add points.
        </p>
      </div>
    )
  }

  return (
    <svg
      ref={svgRef}
      width={CHART_WIDTH}
      height={CHART_HEIGHT}
      style={{ display: 'block', margin: '0 auto', backgroundColor: 'white' }}
      onMouseMove={handleMouseMove}
      onMouseUp={handleMouseUp}
      onMouseLeave={handleMouseUp}
    >
      <rect
        x={PADDING.left}
        y={PADDING.top}
        width={INNER_WIDTH}
        height={INNER_HEIGHT}
        fill="#f8f9fa"
        stroke="#e0e0e0"
      />

      {xTicks.map((tick) => (
        <line
          key={`x-grid-${tick}`}
          x1={scaleX(tick)}
          y1={PADDING.top}
          x2={scaleX(tick)}
          y2={PADDING.top + INNER_HEIGHT}
          stroke="#e0e0e0"
          strokeDasharray="4,4"
        />
      ))}
      {yTicks.map((tick) => (
        <line
          key={`y-grid-${tick}`}
          x1={PADDING.left}
          y1={scaleY(tick)}
          x2={PADDING.left + INNER_WIDTH}
          y2={scaleY(tick)}
          stroke="#e0e0e0"
          strokeDasharray="4,4"
        />
      ))}

      <line
        x1={PADDING.left}
        y1={PADDING.top + INNER_HEIGHT}
        x2={PADDING.left + INNER_WIDTH}
        y2={PADDING.top + INNER_HEIGHT}
        stroke="#333"
        strokeWidth="2"
      />
      {xTicks.map((tick) => (
        <g key={`x-tick-${tick}`}>
          <line
            x1={scaleX(tick)}
            y1={PADDING.top + INNER_HEIGHT}
            x2={scaleX(tick)}
            y2={PADDING.top + INNER_HEIGHT + 5}
            stroke="#333"
          />
          <text
            x={scaleX(tick)}
            y={PADDING.top + INNER_HEIGHT + 18}
            textAnchor="middle"
            fontSize="11"
            fill="#666"
          >
            {tick}
          </text>
        </g>
      ))}
      <text
        x={PADDING.left + INNER_WIDTH / 2}
        y={CHART_HEIGHT - 8}
        textAnchor="middle"
        fontSize="12"
        fill="#333"
        fontWeight="500"
      >
        {xLabel}
      </text>

      <line
        x1={PADDING.left}
        y1={PADDING.top}
        x2={PADDING.left}
        y2={PADDING.top + INNER_HEIGHT}
        stroke="#333"
        strokeWidth="2"
      />
      {yTicks.map((tick) => (
        <g key={`y-tick-${tick}`}>
          <line
            x1={PADDING.left - 5}
            y1={scaleY(tick)}
            x2={PADDING.left}
            y2={scaleY(tick)}
            stroke="#333"
          />
          <text
            x={PADDING.left - 10}
            y={scaleY(tick) + 4}
            textAnchor="end"
            fontSize="11"
            fill="#666"
          >
            {tick}
          </text>
        </g>
      ))}
      <text
        x={15}
        y={PADDING.top + INNER_HEIGHT / 2}
        textAnchor="middle"
        fontSize="12"
        fill="#333"
        fontWeight="500"
        transform={`rotate(-90, 15, ${PADDING.top + INNER_HEIGHT / 2})`}
      >
        {yLabel}
      </text>

      <path
        d={linePath}
        fill="none"
        stroke="#3498db"
        strokeWidth="2"
        strokeLinejoin="round"
      />

      {points.map((point, pointIndex) => {
        const isHovered = hoveredPoint === pointIndex
        const isDragging = draggingPoint === pointIndex
        const radius = isHovered || isDragging ? 8 : 6

        return (
          <g key={pointIndex}>
            <circle
              cx={scaleX(point.x)}
              cy={scaleY(point.y)}
              r={radius}
              fill={isDragging ? '#e74c3c' : '#3498db'}
              stroke="#fff"
              strokeWidth="2"
              style={{ cursor: 'grab' }}
              onMouseDown={(e) => handleMouseDown(e, pointIndex)}
              onMouseEnter={() => setHoveredPoint(pointIndex)}
              onMouseLeave={() => setHoveredPoint(null)}
            />
            {(isHovered || isDragging) && (
              <text
                x={scaleX(point.x)}
                y={scaleY(point.y) - 12}
                textAnchor="middle"
                fontSize="11"
                fill="#333"
                fontWeight="500"
              >
                ({Math.round(point.x)}, {Math.round(point.y)})
              </text>
            )}
          </g>
        )
      })}
    </svg>
  )
}

export default CurveChart
