import { useEffect, useState } from 'react'
import { AccessibleDialog, DialogFooter } from '@/components/ui/dialog'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Alert, AlertDescription } from '@/components/ui/alert'
import { Label } from '@/components/ui/label'

interface SaveAsModalProps {
  isOpen: boolean
  /** Initial value populated into the name field. The user is free to edit. */
  defaultName: string
  /** When true, render a banner explaining we're branching off a read-only seed. */
  fromSeed: boolean
  onClose: () => void
  /**
   * Confirm handler. Receives the sanitized name (already passed through the
   * same `[^a-zA-Z0-9_-]/g -> _` filter the rest of App.tsx uses, so callers
   * don't need to re-sanitize).
   */
  onConfirm: (name: string) => void
}

const sanitize = (raw: string): string =>
  raw.trim().replace(/[^a-zA-Z0-9_-]/g, '_')

// Single source of truth for the seed-branch warning copy. The DialogDescription
// uses it for a11y announcement; the inline Alert repeats it for visual emphasis.
const SEED_WARNING_MESSAGE =
  'Saving as a new profile because the loaded one is a read-only seed.'
const DEFAULT_DESCRIPTION = 'Enter a name for the profile.'

/**
 * Save-as prompt used when the loaded profile cannot be saved in place —
 * either because nothing is loaded yet or because the loaded entry is a
 * read-only seed (Phase 3 of #222). Replaces the `prompt()` dialog the rest
 * of App.tsx uses for one-off names; that one is fine for transient flows
 * but the seed-branch case wants the explanatory banner.
 */
function SaveAsModal({
  isOpen,
  defaultName,
  fromSeed,
  onClose,
  onConfirm,
}: SaveAsModalProps) {
  const [name, setName] = useState(defaultName)

  // Re-seed the input every time the modal opens, so subsequent invocations
  // don't carry stale typing from a previous cancel.
  useEffect(() => {
    if (isOpen) {
      setName(defaultName)
    }
  }, [isOpen, defaultName])

  const sanitized = sanitize(name)
  const canSubmit = sanitized.length > 0

  const handleSubmit = (event: React.FormEvent) => {
    event.preventDefault()
    if (!canSubmit) return
    onConfirm(sanitized)
  }

  return (
    <AccessibleDialog
      isOpen={isOpen}
      onClose={onClose}
      title="Save Profile As"
      description={fromSeed ? SEED_WARNING_MESSAGE : DEFAULT_DESCRIPTION}
      contentClassName="sm:max-w-md"
    >
      <form onSubmit={handleSubmit} className="flex flex-col gap-4">
        {fromSeed && (
          <Alert>
            <AlertDescription>{SEED_WARNING_MESSAGE}</AlertDescription>
          </Alert>
        )}

        <div className="flex flex-col gap-2">
          <Label htmlFor="save-as-name">Profile name</Label>
          <Input
            id="save-as-name"
            autoFocus
            value={name}
            onChange={(e) => setName(e.target.value)}
            placeholder="my_profile"
          />
          {name && sanitized !== name.trim() && (
            <p className="text-xs text-muted-foreground">
              Will be saved as <code>{sanitized}.json</code>
            </p>
          )}
        </div>

        <DialogFooter>
          <Button type="button" variant="outline" onClick={onClose}>
            Cancel
          </Button>
          <Button type="submit" disabled={!canSubmit}>
            Save
          </Button>
        </DialogFooter>
      </form>
    </AccessibleDialog>
  )
}

export default SaveAsModal
