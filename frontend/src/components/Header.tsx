import { useRef } from 'react'
import { Button } from '@/components/ui/button'
import { Separator } from '@radix-ui/themes'
import { GearIcon } from '@radix-ui/react-icons'

interface HeaderProps {
  onImportFile: (file: File) => void
  onLoadFromServer: () => void
  onSave: () => void
  onCopy: () => void
  onExport: () => void
  onNewProfile: () => void
  onOpenPreferences: () => void
  onValidate: () => void
  profileName: string | null
  isModified: boolean
}

function Header({
  onImportFile: onImportFile,
  onLoadFromServer,
  onSave,
  onCopy,
  onExport,
  onNewProfile,
  onOpenPreferences,
  onValidate,
  profileName,
  isModified,
}: HeaderProps) {
  const fileInputRef = useRef<HTMLInputElement>(null)

  const handleFileImport = (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0]
    if (file) {
      onImportFile(file)
      e.target.value = ''
    }
  }

  const handleImportClick = () => {
    if (isModified) {
      if (
        !confirm(
          'You have unsaved changes. Are you sure you want to import a new profile?',
        )
      ) {
        return
      }
    }
    fileInputRef.current?.click()
  }

  return (
    <header className="pb-2 border-b border-border">
      <div className="flex items-center justify-between">
        <div>
          <h3 className="text-lg font-semibold">1815.2 Test Tool</h3>
          <p className="text-sm text-muted-foreground">
            Current Profile: {profileName || <em>no profile loaded</em>}
          </p>
        </div>
        <div className="flex items-center gap-1">
          <Button
            size="sm"
            onClick={onNewProfile}
            title="Create new profile (Alt+N)"
          >
            New Profile
          </Button>
          <Button
            size="sm"
            onClick={onLoadFromServer}
            title="Load profile from server (Alt+L)"
          >
            Load Profile
          </Button>
          <Button
            size="sm"
            onClick={handleImportClick}
            title="Import profile from file (Alt+I)"
          >
            Import Profile
          </Button>
          <Button
            size="sm"
            className="bg-success text-white hover:bg-success/90"
            onClick={onSave}
            disabled={!isModified}
            title={
              isModified
                ? 'Save changes to current profile (Ctrl+S)'
                : 'No changes to save'
            }
          >
            Save
          </Button>
          <Button
            variant="outline"
            size="sm"
            onClick={onExport}
            title="Export profile to local file (Alt+E)"
          >
            Export
          </Button>
          <Button
            variant="outline"
            size="sm"
            onClick={onValidate}
            title="Re-run validation against the current profile"
          >
            Validate
          </Button>
          <Button
            variant="outline"
            size="sm"
            onClick={onCopy}
            title="Copy profile with new name"
          >
            Copy
          </Button>
          <Separator orientation="vertical"></Separator>
          <Button
            variant="outline"
            color="gray"
            size="sm"
            onClick={onOpenPreferences}
            title="Open preferences"
          >
            <GearIcon /> Settings
          </Button>
          <input
            id="file-input"
            ref={fileInputRef}
            type="file"
            accept=".json"
            onChange={handleFileImport}
            className="hidden"
          />
        </div>
      </div>
    </header>
  )
}

export default Header
