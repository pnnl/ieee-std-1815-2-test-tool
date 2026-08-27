import { Badge } from '@/components/ui/badge'

interface ValidationErrorBadgeProps {
  count: number
}

// Shared by the tab-level and offset-group-level badges so routed errors
// are visible without opening every tab or expanding every group. Callers
// gate on `count > 0`, so a zero count never mounts this.
function ValidationErrorBadge({ count }: ValidationErrorBadgeProps) {
  return (
    <Badge
      variant="destructive"
      className="h-4 min-w-4 justify-center rounded-full px-1 text-[10px] leading-4"
    >
      <span aria-hidden="true">{count}</span>
      <span className="sr-only">
        {count} validation {count === 1 ? 'error' : 'errors'}
      </span>
    </Badge>
  )
}

export default ValidationErrorBadge
