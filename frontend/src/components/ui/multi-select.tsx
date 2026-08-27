import * as React from 'react'
import { useState, useRef, useEffect } from 'react'
import { Popover as PopoverPrimitive } from 'radix-ui'
import { XIcon, ChevronDownIcon, CheckIcon } from 'lucide-react'
import { cn } from '@/lib/utils'
import { Badge } from '@/components/ui/badge'
import { Label } from '@/components/ui/label'

interface MultiSelectOption {
  value: string
  label: string
}

interface MultiSelectProps {
  label?: string
  placeholder?: string
  options: MultiSelectOption[]
  selected: string[]
  onChange: (selected: string[]) => void
  className?: string
}

function MultiSelect({
  label,
  placeholder = 'Filter...',
  options,
  selected,
  onChange,
  className,
}: MultiSelectProps) {
  const [open, setOpen] = useState(false)
  const [search, setSearch] = useState('')
  const inputRef = useRef<HTMLInputElement>(null)

  const filtered = options.filter((opt) =>
    opt.label.toLowerCase().includes(search.toLowerCase()),
  )

  const toggle = (value: string) => {
    if (selected.includes(value)) {
      onChange(selected.filter((v) => v !== value))
    } else {
      onChange([...selected, value])
    }
  }

  const removeTag = (value: string, e: React.MouseEvent) => {
    e.stopPropagation()
    onChange(selected.filter((v) => v !== value))
  }

  const clearAll = (e: React.MouseEvent) => {
    e.stopPropagation()
    onChange([])
    setSearch('')
  }

  // Focus input when popover opens
  useEffect(() => {
    if (open) {
      setTimeout(() => inputRef.current?.focus(), 0)
    } else {
      setSearch('')
    }
  }, [open])

  const selectedLabels = selected.map((v) => {
    const opt = options.find((o) => o.value === v)
    return opt ? opt.label : v
  })

  return (
    <div className={cn('flex flex-col gap-1', className)}>
      {label && <Label className="text-xs font-medium">{label}</Label>}
      <PopoverPrimitive.Root open={open} onOpenChange={setOpen}>
        <PopoverPrimitive.Trigger asChild>
          <button
            type="button"
            className={cn(
              'flex items-center gap-1 min-h-8 w-full rounded-md border border-input bg-background px-2 py-1 text-sm',
              'ring-offset-background focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-1',
              'cursor-pointer hover:bg-muted/50',
            )}
          >
            <div className="flex flex-1 flex-wrap items-center gap-1 min-w-0">
              {selected.length === 0 && (
                <span className="text-muted-foreground text-sm">
                  {placeholder}
                </span>
              )}
              {selectedLabels.map((lbl, i) => (
                <Badge
                  key={selected[i]}
                  variant="secondary"
                  className="text-xs px-1.5 py-0 gap-0.5 shrink-0"
                >
                  {lbl}
                  <span
                    role="button"
                    tabIndex={-1}
                    className="ml-0.5 hover:text-destructive cursor-pointer"
                    onPointerDown={(e) => e.stopPropagation()}
                    onClick={(e) => removeTag(selected[i], e)}
                  >
                    <XIcon className="size-3" />
                  </span>
                </Badge>
              ))}
            </div>
            <div className="flex items-center gap-0.5 shrink-0 ml-1">
              {selected.length > 0 && (
                <span
                  role="button"
                  tabIndex={-1}
                  className="text-muted-foreground hover:text-foreground cursor-pointer"
                  onPointerDown={(e) => e.stopPropagation()}
                  onClick={clearAll}
                >
                  <XIcon className="size-3.5" />
                </span>
              )}
              <ChevronDownIcon className="size-4 text-muted-foreground" />
            </div>
          </button>
        </PopoverPrimitive.Trigger>

        <PopoverPrimitive.Portal>
          <PopoverPrimitive.Content
            align="start"
            sideOffset={4}
            className={cn(
              'z-50 w-[var(--radix-popover-trigger-width)] rounded-md border bg-popover shadow-md outline-none',
              'data-[state=open]:animate-in data-[state=closed]:animate-out',
              'data-[state=closed]:fade-out-0 data-[state=open]:fade-in-0',
              'data-[state=closed]:zoom-out-95 data-[state=open]:zoom-in-95',
            )}
          >
            <div className="p-1.5 border-b">
              <input
                ref={inputRef}
                type="text"
                value={search}
                onChange={(e) => setSearch(e.target.value)}
                placeholder="Type to search..."
                className="w-full bg-transparent text-sm outline-none placeholder:text-muted-foreground"
              />
            </div>
            <div className="max-h-[200px] overflow-auto p-1">
              {filtered.length === 0 && (
                <div className="py-2 px-2 text-sm text-muted-foreground text-center">
                  No results
                </div>
              )}
              {filtered.map((opt) => {
                const isSelected = selected.includes(opt.value)
                return (
                  <button
                    key={opt.value}
                    type="button"
                    onClick={() => toggle(opt.value)}
                    className={cn(
                      'flex items-center gap-2 w-full rounded-sm px-2 py-1.5 text-sm cursor-pointer',
                      'hover:bg-accent hover:text-accent-foreground',
                      isSelected && 'bg-accent/50',
                    )}
                  >
                    <div
                      className={cn(
                        'flex items-center justify-center size-4 rounded-sm border',
                        isSelected
                          ? 'bg-primary border-primary text-primary-foreground'
                          : 'border-input',
                      )}
                    >
                      {isSelected && <CheckIcon className="size-3" />}
                    </div>
                    <span className="truncate">{opt.label}</span>
                  </button>
                )
              })}
            </div>
          </PopoverPrimitive.Content>
        </PopoverPrimitive.Portal>
      </PopoverPrimitive.Root>
    </div>
  )
}

export { MultiSelect }
