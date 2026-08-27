// Dialog listing the errors from an import that produced no profile.
// Dismissible, and states plainly that the file was not loaded so the
// previously loaded profile is not implicated.

import { Dialog, Button } from '@radix-ui/themes'
import type { ValidationError } from '@/api/generated'

interface ImportErrorDialogProps {
  open: boolean
  errors: readonly ValidationError[]
  onClose: () => void
}

function ImportErrorDialog({ open, errors, onClose }: ImportErrorDialogProps) {
  return (
    <Dialog.Root
      open={open}
      onOpenChange={(isOpen) => {
        if (!isOpen) onClose()
      }}
    >
      <Dialog.Content maxWidth="480px">
        <Dialog.Title>Import failed</Dialog.Title>
        <Dialog.Description size="2" mb="3">
          The file was not loaded. The profile currently on screen, if any, is
          unchanged.
        </Dialog.Description>

        <ul className="list-disc pl-4 space-y-1 max-h-[50vh] overflow-y-auto">
          {errors.map((error, index) => (
            <li key={`${error.point}|${error.message}|${index}`}>
              {error.point ? (
                <span className="font-mono">{error.point}: </span>
              ) : null}
              {error.message}
            </li>
          ))}
        </ul>

        <div className="flex justify-end mt-4">
          {/* No explicit onClick here: Dialog.Close already triggers
              onOpenChange(false) above, which calls onClose once. An
              explicit onClick={onClose} here fired it twice. */}
          <Dialog.Close>
            <Button variant="soft">Close</Button>
          </Dialog.Close>
        </div>
      </Dialog.Content>
    </Dialog.Root>
  )
}

export default ImportErrorDialog
