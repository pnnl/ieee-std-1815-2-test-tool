import * as React from 'react'
import { cva, type VariantProps } from 'class-variance-authority'

import { cn } from '@/lib/utils'

const toggleGroupVariants = cva('inline-flex rounded-md border border-input', {
  variants: {
    size: {
      default: '',
      sm: '',
    },
  },
  defaultVariants: {
    size: 'default',
  },
})

const toggleGroupItemVariants = cva(
  'relative inline-flex flex-1 items-center justify-center whitespace-nowrap text-sm font-medium transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-1 disabled:pointer-events-none disabled:opacity-50 cursor-pointer first:rounded-l-md last:rounded-r-md not-last:border-r border-input',
  {
    variants: {
      size: {
        default: 'h-9 px-4 py-2',
        sm: 'h-8 px-3 py-1.5 text-xs',
      },
      active: {
        true: 'bg-primary text-primary-foreground hover:bg-primary/90',
        false:
          'bg-background text-foreground hover:bg-accent hover:text-accent-foreground opacity-50',
      },
    },
    defaultVariants: {
      size: 'default',
      active: false,
    },
  },
)

interface ToggleGroupContextValue {
  value: string
  onValueChange: (value: string) => void
  size?: 'default' | 'sm'
}

const ToggleGroupContext = React.createContext<ToggleGroupContextValue | null>(
  null,
)

function useToggleGroupContext() {
  const ctx = React.useContext(ToggleGroupContext)
  if (!ctx) throw new Error('ToggleGroupItem must be used inside ToggleGroup')
  return ctx
}

function ToggleGroup({
  className,
  size = 'default',
  value,
  onValueChange,
  children,
  ...props
}: React.ComponentProps<'div'> &
  VariantProps<typeof toggleGroupVariants> & {
    value: string
    onValueChange: (value: string) => void
  }) {
  return (
    <ToggleGroupContext.Provider
      value={{ value, onValueChange, size: size ?? 'default' }}
    >
      <div
        data-slot="toggle-group"
        role="group"
        className={cn(toggleGroupVariants({ size }), className)}
        {...props}
      >
        {children}
      </div>
    </ToggleGroupContext.Provider>
  )
}

function ToggleGroupItem({
  className,
  value,
  children,
  ...props
}: React.ComponentProps<'button'> & { value: string }) {
  const { value: groupValue, onValueChange, size } = useToggleGroupContext()
  const isActive = groupValue === value

  return (
    <button
      data-slot="toggle-group-item"
      data-state={isActive ? 'on' : 'off'}
      aria-pressed={isActive}
      type="button"
      onClick={() => onValueChange(value)}
      className={cn(
        toggleGroupItemVariants({ size, active: isActive }),
        className,
      )}
      {...props}
    >
      {children}
    </button>
  )
}

export { ToggleGroup, ToggleGroupItem }
