import * as React from 'react'
import { cn } from '@/lib/utils'
import { Label } from '@/components/ui/label'

interface SelectOption {
  value: string
  label: string
}

interface NativeSelectProps {
  id?: string
  label?: string
  value?: string
  onChange?: (e: React.ChangeEvent<HTMLSelectElement>) => void
  data: SelectOption[]
  size?: 'xs' | 'sm' | 'md'
  disabled?: boolean
  className?: string
}

const NativeSelect = React.forwardRef<HTMLSelectElement, NativeSelectProps>(
  (
    { id, label, value, onChange, data, size = 'sm', disabled, className },
    ref,
  ) => {
    const generatedId = React.useId()
    const selectId = id ?? generatedId

    const sizeClasses = {
      xs: 'h-7 text-xs px-2',
      sm: 'h-8 text-sm px-3',
      md: 'h-9 text-sm px-3',
    }

    return (
      <div className={cn('flex flex-col gap-1', className)}>
        {label && (
          <Label htmlFor={selectId} className="text-xs font-medium">
            {label}
          </Label>
        )}
        <select
          ref={ref}
          id={selectId}
          value={value}
          onChange={onChange}
          disabled={disabled}
          className={cn(
            'flex w-full rounded-md border border-input bg-background ring-offset-background',
            'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-1',
            'disabled:cursor-not-allowed disabled:opacity-50 cursor-pointer',
            sizeClasses[size],
          )}
        >
          {data.map((opt) => (
            <option key={opt.value} value={opt.value}>
              {opt.label}
            </option>
          ))}
        </select>
      </div>
    )
  },
)
NativeSelect.displayName = 'NativeSelect'

export { NativeSelect }
