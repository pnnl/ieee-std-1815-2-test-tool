import * as React from 'react'
import { cn } from '@/lib/utils'
import { Label } from '@/components/ui/label'

interface NumberInputProps {
  id?: string
  label?: string
  description?: string
  value?: number | string
  onChange?: (value: number | string) => void | Promise<void>
  commitOnBlur?: boolean
  min?: number
  max?: number
  step?: number
  disabled?: boolean
  size?: 'xs' | 'sm' | 'md'
  error?: string
  className?: string
  styles?: { input?: React.CSSProperties }
  hideControls?: boolean
}

const NumberInput = React.forwardRef<HTMLInputElement, NumberInputProps>(
  (
    {
      label,
      description,
      value,
      onChange,
      commitOnBlur = false,
      min,
      max,
      step = 1,
      disabled,
      size = 'sm',
      error,
      className,
      styles,
      hideControls,
      ...props
    },
    ref,
  ) => {
    const generatedId = React.useId()
    const inputId = props.id ?? generatedId
    const sizeClasses = {
      xs: 'h-7 text-xs px-2',
      sm: 'h-8 text-sm px-3',
      md: 'h-9 text-sm px-3',
    }

    // Track the raw display string so users can clear/edit freely
    const [displayValue, setDisplayValue] = React.useState(() =>
      value != null && value !== '' ? String(value) : '',
    )
    const [isFocused, setIsFocused] = React.useState(false)
    const [isCommitting, setIsCommitting] = React.useState(false)

    // Keep the draft visible while editing and while an async commit is pending.
    React.useEffect(() => {
      if (!isFocused && !isCommitting) {
        setDisplayValue(value != null && value !== '' ? String(value) : '')
      }
    }, [value, isFocused, isCommitting])

    const handleChange = (e: React.ChangeEvent<HTMLInputElement>) => {
      const raw = e.target.value
      setDisplayValue(raw)
      if (commitOnBlur) return

      if (raw === '' || raw === '-') {
        onChange?.('')
        return
      }
      const num = parseFloat(raw)
      if (!isNaN(num)) {
        onChange?.(num)
      }
    }

    const handleBlur = async () => {
      setIsFocused(false)
      if (commitOnBlur) {
        const num = Number(displayValue)
        if (
          displayValue.trim() !== '' &&
          Number.isFinite(num) &&
          (value == null || value === '' || num !== Number(value))
        ) {
          setIsCommitting(true)
          try {
            await onChange?.(num)
          } finally {
            setIsCommitting(false)
          }
        }
        return
      }
      // On blur, normalize the display to match the actual value
      if (displayValue === '' || displayValue === '-') {
        setDisplayValue(value != null && value !== '' ? String(value) : '')
      }
    }

    return (
      <div className={cn('flex flex-col gap-1', className)}>
        {label && (
          <Label htmlFor={inputId} className="text-xs font-medium">
            {label}
          </Label>
        )}
        {description && (
          <p className="text-xs text-muted-foreground -mt-0.5">{description}</p>
        )}
        <input
          ref={ref}
          id={inputId}
          type="number"
          value={displayValue}
          onChange={handleChange}
          onFocus={() => setIsFocused(true)}
          onBlur={handleBlur}
          onKeyDown={(e) => {
            if (commitOnBlur && e.key === 'Enter') {
              e.preventDefault()
              e.currentTarget.blur()
            }
          }}
          min={min}
          max={max}
          step={step}
          disabled={disabled}
          className={cn(
            'flex w-full rounded-md border border-input bg-background ring-offset-background',
            'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-1',
            'disabled:cursor-not-allowed disabled:opacity-50',
            sizeClasses[size],
            error && 'border-destructive',
            hideControls &&
              '[appearance:textfield] [&::-webkit-outer-spin-button]:appearance-none [&::-webkit-inner-spin-button]:appearance-none',
          )}
          style={styles?.input}
          {...props}
        />
        {error && <p className="text-xs text-destructive">{error}</p>}
      </div>
    )
  },
)
NumberInput.displayName = 'NumberInput'

export { NumberInput }
