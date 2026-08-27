import { useState, useEffect, useCallback } from 'react'
import { AccessibleDialog } from '@/components/ui/dialog'
import { Button } from '@/components/ui/button'
import { Alert, AlertTitle, AlertDescription } from '@/components/ui/alert'
import {
  Table,
  TableHeader,
  TableBody,
  TableRow,
  TableHead,
  TableCell,
} from '@/components/ui/table'
import {
  listProfiles,
  type ProfileListItem,
  type ProfileSource,
} from '@/api/generated'

// Local view-model shape kept stable for the existing render code below.
// Sourced from `ProfileListItem` (codegen, utoipa-emitted spec): the wire
// field is `last_modified` (Option<u64> epoch seconds); we convert to ISO
// here so the existing `formatDate` helper keeps working unchanged.
interface SimpleProfile {
  name: string
  filename: string
  modified: string
  source: ProfileSource
}

interface ProfileListModalProps {
  isOpen: boolean
  onClose: () => void
  onSelect: (profileName: string) => void
  mode?: 'load' | 'export'
}

// Section metadata for the rendered groups. The order here is the rendered
// order — seeds first because they're the curated stable set, working second
// because that's the user's working area.
const SECTIONS: ReadonlyArray<{
  source: ProfileSource
  title: string
  emptyHint: string
}> = [
  {
    source: 'seed',
    title: 'Seeds',
    emptyHint: 'No seed profiles available.',
  },
  {
    source: 'working',
    title: 'My Profiles',
    emptyHint: 'No saved profiles yet.',
  },
]

function ProfileListModal({
  isOpen,
  onClose,
  onSelect,
  mode = 'load',
}: ProfileListModalProps) {
  const [profiles, setProfiles] = useState<SimpleProfile[]>([])
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)

  const loadProfiles = useCallback(async () => {
    setLoading(true)
    setError(null)
    try {
      // Generated client (Phase 2 of #225). `baseUrl: ''` forces same-origin
      // so the request flows through the Vite split-route proxy to Poem.
      const { data, error: apiError } = await listProfiles({
        baseUrl: '',
        throwOnError: false,
      })
      if (apiError || !data) {
        throw new Error(
          apiError
            ? `Failed to load profiles (${String(apiError)})`
            : 'Failed to load profiles',
        )
      }
      const items: ProfileListItem[] = data
      setProfiles(
        items.map((p) => ({
          name: p.name,
          filename: p.filename,
          modified:
            p.last_modified != null
              ? new Date(p.last_modified * 1000).toISOString()
              : '',
          source: p.source,
        })),
      )
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Unknown error')
    } finally {
      setLoading(false)
    }
  }, [])

  useEffect(() => {
    if (isOpen) {
      loadProfiles()
    }
  }, [isOpen, loadProfiles])

  const handleSelect = (profileName: string) => {
    onSelect(profileName)
    onClose()
  }

  const formatDate = (dateString: string) => {
    if (!dateString) return '-'
    const date = new Date(dateString)
    return date.toLocaleString()
  }

  const modalTitle = mode === 'export' ? 'Export Profile' : 'Load Profile'
  const actionButtonText = mode === 'export' ? 'Export' : 'Load'

  return (
    <AccessibleDialog
      isOpen={isOpen}
      onClose={onClose}
      title={modalTitle}
      description={
        mode === 'export'
          ? 'Select a profile to export.'
          : 'Select a profile to load.'
      }
      contentClassName="sm:max-w-2xl max-h-[80vh] overflow-auto"
      footer={
        <Button variant="outline" onClick={onClose}>
          Cancel
        </Button>
      }
    >
      <div className="flex flex-col gap-4">
        {loading && (
          <div className="flex items-center justify-center gap-2 p-4">
            <div className="h-4 w-4 animate-spin rounded-full border-2 border-primary border-t-transparent" />
            <p className="text-sm">Loading profiles...</p>
          </div>
        )}

        {error && (
          <Alert variant="destructive">
            <AlertTitle>Error</AlertTitle>
            <AlertDescription>{error}</AlertDescription>
          </Alert>
        )}

        {!loading && !error && profiles.length === 0 && (
          <p className="text-muted-foreground text-center p-4">
            No profiles found on server.
          </p>
        )}

        {!loading && !error && profiles.length > 0 && (
          <div className="flex flex-col gap-6">
            {SECTIONS.map(({ source, title, emptyHint }) => {
              // The backend already deduplicates: working shadows seed on
              // name collision, so a name appears under at most one source
              // here. That makes the partition trivial — straight filter,
              // no merge logic.
              const sectionProfiles = profiles.filter(
                (p) => p.source === source,
              )
              return (
                <section
                  key={source}
                  aria-label={title}
                  className="flex flex-col gap-2"
                >
                  <h3 className="text-sm font-semibold text-muted-foreground uppercase tracking-wide">
                    {title}
                  </h3>
                  {sectionProfiles.length === 0 ? (
                    <p className="text-sm text-muted-foreground italic px-1">
                      {emptyHint}
                    </p>
                  ) : (
                    <Table>
                      <TableHeader>
                        <TableRow>
                          <TableHead>Profile Name</TableHead>
                          <TableHead>Last Modified</TableHead>
                          <TableHead>Action</TableHead>
                        </TableRow>
                      </TableHeader>
                      <TableBody>
                        {sectionProfiles.map((profile) => (
                          <TableRow
                            key={profile.filename}
                            className="hover:bg-muted/50"
                          >
                            <TableCell>{profile.name}</TableCell>
                            <TableCell>
                              {formatDate(profile.modified)}
                            </TableCell>
                            <TableCell>
                              <Button
                                size="xs"
                                variant="secondary"
                                onClick={() => handleSelect(profile.name)}
                              >
                                {actionButtonText}
                              </Button>
                            </TableCell>
                          </TableRow>
                        ))}
                      </TableBody>
                    </Table>
                  )}
                </section>
              )
            })}
          </div>
        )}
      </div>
    </AccessibleDialog>
  )
}

export default ProfileListModal
