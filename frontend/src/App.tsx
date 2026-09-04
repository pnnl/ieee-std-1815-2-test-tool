import { useState, useEffect, useCallback, useMemo, useRef } from 'react'
import { useSearchParams } from 'react-router-dom'
import { toast } from 'sonner'
import Header from './components/Header'
import Tabs from './components/Tabs'
import EntitiesTab from './components/EntitiesTab'
import PointsTab from './components/PointsTab'
import SchedulingTab from './components/SchedulingTab/SchedulingTab'
import CurvesTab from './components/CurvesTab'
import TestRunnerTab from './components/test-runner-tab/TestRunnerTab'
import ProfileListModal from './components/ProfileListModal'
import SaveAsModal from './components/SaveAsModal'
import {
  isPointSection,
  setEquipmentCount,
  getEquipmentCount,
  updatePoint,
  type EquipmentGroup,
  type FlatPoint,
} from './profile/canonical'
import { bucketValidationErrors } from './profile/validationBucketing'
import { interpretValidateError } from './profile/importResponse'
import {
  loadPreferences,
  updatePreference,
  Preferences,
} from './utils/preferences'
import PreferencesModal from './components/PreferencesModal'
import ValidationErrorCallout from './components/ValidationErrorCallout'
import ImportErrorDialog from './components/ImportErrorDialog'
import {
  getProfile,
  listProfiles,
  saveProfile,
  validateProfile,
  type PicsProfile,
  type ProfileSource,
  type ValidationError,
} from '@/api/generated'
import { EnumDataProvider } from './contexts/EnumDataProvider'

// Default seed loaded on first session. The `full` profile
// is under `data/profiles/` and is reported by the API as `source: seed`.
const DEFAULT_PROFILE_NAME = 'full'

// localStorage key tracking the most recently loaded profile name. Reset to
// `null` (key removed) when the recorded profile fails to load, and the next
// boot then falls back to `DEFAULT_PROFILE_NAME`.
const LAST_LOADED_KEY = 'mesa.lastLoadedProfile'

// Shown when a `/validate` call could not answer the validity question at
// all (an `unconfirmed` outcome), distinct from a clean (empty) error set.
// Shared by both the import path and `loadProfileByName`'s load-time
// re-validation, so the wording must hold for "imported" and "loaded"
// alike. `handleValidateProfile` handles its own unconfirmed case
// separately.
const VALIDATION_UNCONFIRMED_ERROR: ValidationError = {
  point: '',
  message:
    'Profile validation could not be completed; the profile was not confirmed valid.',
}

const tabs = [
  { id: 'entities', label: 'Entities' },
  { id: 'binary_outputs', label: 'Binary Outputs' },
  { id: 'binary_inputs', label: 'Binary Inputs' },
  { id: 'analog_outputs', label: 'Analog Outputs' },
  { id: 'analog_inputs', label: 'Analog Inputs' },
  { id: 'curves', label: 'Curves' },
  { id: 'scheduling', label: 'Scheduling' },
  { id: 'test_runner', label: 'Test Runner' },
]

const getTabFromSearch = (searchParams: URLSearchParams): string => {
  const tab = searchParams.get('tab')
  const isValidTab = tabs.some((item) => item.id === tab)
  return isValidTab ? tab! : 'test_runner'
}

function App() {
  const [profileData, setProfileData] = useState<PicsProfile | null>(null)
  const [searchParams, setSearchParams] = useSearchParams()
  const activeTab = getTabFromSearch(searchParams)
  const [currentProfileName, setCurrentProfileName] = useState<string | null>(
    null,
  )
  // Provenance of the currently loaded profile. `null` means nothing has been
  // loaded yet (pre-bootstrap or after a load failure). Drives the Save
  // button's branch: working-sourced profiles save in place; seed-sourced
  // profiles (or `null`) prompt for a new name.
  const [loadedSource, setLoadedSource] = useState<ProfileSource | null>(null)
  const [isModified, setIsModified] = useState(false)
  const [originalProfileData, setOriginalProfileData] =
    useState<PicsProfile | null>(null)
  const [showLoadModal, setShowLoadModal] = useState(false)
  const [showPreferencesModal, setShowPreferencesModal] = useState(false)
  const [showSaveAsModal, setShowSaveAsModal] = useState(false)
  const [purposeFilter, setPurposeFilter] = useState<string[]>([])
  const [preferences, setPreferences] = useState<Preferences>(() =>
    loadPreferences(),
  )

  // Errors about the LOADED profile (General panel + per-tab callouts).
  // Replaced wholesale on every load/import/Validate event, never patched
  // incrementally, so a previous profile's errors can't bleed into a new one.
  const [validationErrors, setValidationErrors] = useState<ValidationError[]>(
    [],
  )
  // Errors about an import that produced NO profile: a dismissible popup,
  // separate from the persistent panel above; the two never overlap.
  const [importFailureErrors, setImportFailureErrors] = useState<
    ValidationError[] | null
  >(null)

  const validationBuckets = useMemo(
    () => bucketValidationErrors(validationErrors),
    [validationErrors],
  )

  // Per-tab counts for the Tabs bar badges, derived from the same
  // `validationBuckets` the panels render from. Point tabs plus 'curves'
  // and 'scheduling' get a bucket; 'entities'/'test_runner' get none.
  const tabErrorCounts = useMemo(() => {
    const counts: Record<string, number> = {}
    for (const tab of tabs) {
      if (
        isPointSection(tab.id) ||
        tab.id === 'curves' ||
        tab.id === 'scheduling'
      ) {
        counts[tab.id] = validationBuckets[tab.id].length
      }
    }
    return counts
  }, [validationBuckets])

  const visitedTabs = useRef<Set<string>>(new Set([activeTab]))

  const setActiveTab = useCallback(
    (tabId: string) => {
      visitedTabs.current.add(tabId)
      setSearchParams((prev) => {
        const next = new URLSearchParams(prev)
        next.set('tab', tabId)
        return next
      })
    },
    [setSearchParams],
  )

  const isTabMounted = (tabId: string) => visitedTabs.current.has(tabId)

  const showToast = (
    message: string,
    type: 'success' | 'error' | 'warning' | 'info' = 'success',
  ) => {
    if (type === 'error') toast.error(message)
    else if (type === 'warning') toast.warning(message)
    else if (type === 'info') toast.info(message)
    else toast.success(message)
  }

  // Track modifications
  useEffect(() => {
    if (profileData && originalProfileData) {
      const hasChanged =
        JSON.stringify(profileData) !== JSON.stringify(originalProfileData)
      setIsModified(hasChanged)
    }
  }, [profileData, originalProfileData])

  // Warn before closing if modified
  useEffect(() => {
    const handleBeforeUnload = (e: BeforeUnloadEvent) => {
      if (isModified) {
        e.preventDefault()
      }
    }

    window.addEventListener('beforeunload', handleBeforeUnload)
    return () => window.removeEventListener('beforeunload', handleBeforeUnload)
  }, [isModified])

  // Persist last-loaded profile.
  useEffect(() => {
    if (currentProfileName) {
      localStorage.setItem(LAST_LOADED_KEY, currentProfileName)
    }
  }, [currentProfileName])

  // Single source of truth for the "this profile was just saved" state
  // transition. Every save site routes through here so the post-save UI is
  // consistent: header name flips, source provenance flips to working,
  // baseline snapshot resets, dirty flag clears, and the localStorage anchor
  // is written eagerly (the currentProfileName effect would also write it,
  // but doing it here keeps the contract self-evident at the call site).
  const markProfileAsSaved = useCallback(
    (name: string, savedProfile: PicsProfile) => {
      setCurrentProfileName(name)
      setLoadedSource('working')
      setOriginalProfileData(structuredClone(savedProfile))
      setIsModified(false)
      try {
        localStorage.setItem(LAST_LOADED_KEY, name)
      } catch {
        // Non-fatal: e.g. private mode quota; the currentProfileName effect
        // will retry on the next render.
      }
    },
    [],
  )

  const handleNewProfile = useCallback(async () => {
    if (isModified) {
      if (
        !confirm(
          'You have unsaved changes. Are you sure you want to create a new profile?',
        )
      ) {
        return
      }
    }

    try {
      const profileName = prompt('Enter a name for the new profile:')

      if (!profileName || profileName.trim() === '') {
        toast.warning('Profile creation cancelled: No name entered')
        return
      }

      let sanitizedNewName = profileName.trim().replace(/[^a-zA-Z0-9_-]/g, '_')

      // Check if profile already exists on server.
      const { error: checkError } = await getProfile({
        baseUrl: '',
        throwOnError: false,
        path: { name: sanitizedNewName },
      })
      const fileExists = !checkError

      if (fileExists) {
        const overwrite = confirm(
          `A profile named "${sanitizedNewName}.json" already exists on the server.\n\n` +
            `Do you want to overwrite it?\n\n` +
            `Click "OK" to overwrite, or "Cancel" to enter a different name.`,
        )

        if (!overwrite) {
          const newName = prompt(
            'Enter a different name for this profile:',
            `${sanitizedNewName}_new`,
          )
          if (!newName || newName.trim() === '') {
            return
          }
          sanitizedNewName = newName.trim().replace(/[^a-zA-Z0-9_-]/g, '_')
        }
      }

      // Use the canonical `full` seed as the new-profile starting template.
      // This replaces the legacy `data/template/profile.json` static fetch.
      const { data: defaultProfile, error: tplError } = await getProfile({
        baseUrl: '',
        throwOnError: false,
        path: { name: DEFAULT_PROFILE_NAME },
      })

      if (tplError || !defaultProfile) {
        throw new Error(
          `Failed to load canonical seed "${DEFAULT_PROFILE_NAME}" from the server.`,
        )
      }

      // Persist FIRST so we never leak partial state on save failure: the
      // previous order set currentProfileName before the POST landed, which
      // then leaked into localStorage via the tracking effect even if the
      // save errored.
      const { error: saveError } = await saveProfile({
        baseUrl: '',
        throwOnError: false,
        body: {
          name: sanitizedNewName,
          profile: defaultProfile,
        },
      })

      if (saveError) {
        showToast('Failed to save new profile to server', 'error')
        return
      }

      setProfileData(defaultProfile)
      setValidationErrors([])
      markProfileAsSaved(sanitizedNewName, defaultProfile)
      showToast(`Profile "${sanitizedNewName}.json" created successfully!`)
    } catch (error: unknown) {
      if (error instanceof Error) {
        showToast('Error creating new profile: ' + error.message, 'error')
        console.error(error)
      } else {
        showToast('Error creating new profile: Unknown error', 'error')
        console.error(error)
      }
    }
  }, [isModified, markProfileAsSaved])

  // Inner helper: actually push the current profile to the working/ dir under
  // the supplied name. Updates app state on success and reports either way
  // through a toast.
  const persistProfile = useCallback(
    async (name: string): Promise<boolean> => {
      if (!profileData) {
        showToast('No profile data to save!', 'warning')
        return false
      }
      // Capture the snapshot we're sending to the server. The user may edit
      // during the await; the saved baseline must reflect what actually
      // hit disk, not whatever profileData has drifted to.
      const snapshot = profileData
      try {
        const { error: saveError } = await saveProfile({
          baseUrl: '',
          throwOnError: false,
          body: {
            name,
            profile: snapshot,
          },
        })

        if (saveError) {
          throw new Error('Failed to save profile to server')
        }

        markProfileAsSaved(name, snapshot)
        showToast(`Profile "${name}.json" saved successfully!`)

        // Save persists regardless of validity, by design. Re-validate the
        // persisted `snapshot` (not a re-fetch) so the panel reflects it;
        // a validate failure here must not read back as "save failed".
        const { error: validateError } = await validateProfile({
          body: snapshot,
        })
        const validateOutcome = interpretValidateError(validateError)
        switch (validateOutcome.kind) {
          case 'clean':
            setValidationErrors([])
            break
          case 'invalid':
            setValidationErrors(validateOutcome.errors)
            break
          case 'unconfirmed':
            // A failed post-save validate must not read as "clean": leave
            // the panel as-is and surface the failure explicitly.
            showToast(
              'Profile saved, but validation could not be re-run; the ' +
                'panel still shows the previous results.',
              'warning',
            )
            break
        }

        return true
      } catch (error: unknown) {
        if (error instanceof Error) {
          showToast('Error saving profile: ' + error.message, 'error')
          console.error(error)
        } else {
          showToast('Error saving profile: Unknown error', 'error')
          console.error(error)
        }
        return false
      }
    },
    [profileData, markProfileAsSaved],
  )

  // Save handler with seed/working branch.
  const handleSaveProfile = useCallback(() => {
    if (!profileData) {
      showToast('No profile data to save!', 'warning')
      return
    }

    if (loadedSource === 'working' && currentProfileName) {
      void persistProfile(currentProfileName)
      return
    }

    setShowSaveAsModal(true)
  }, [profileData, loadedSource, currentProfileName, persistProfile])

  const handleSaveAsConfirm = useCallback(
    async (name: string) => {
      const ok = await persistProfile(name)
      if (ok) {
        setShowSaveAsModal(false)
      }
    },
    [persistProfile],
  )

  const handleExportProfile = useCallback(() => {
    if (!profileData) {
      showToast('No profile loaded to export', 'warning')
      return
    }

    try {
      const fileName = currentProfileName || 'profile'
      const jsonString = JSON.stringify(profileData, null, 2)
      const blob = new Blob([jsonString], { type: 'application/json' })

      const url = URL.createObjectURL(blob)
      const link = document.createElement('a')
      link.href = url
      link.download = `${fileName}.json`

      document.body.appendChild(link)
      link.click()

      document.body.removeChild(link)
      URL.revokeObjectURL(url)

      showToast(`Profile "${fileName}.json" exported successfully!`)
    } catch (error: unknown) {
      if (error instanceof Error) {
        showToast('Error exporting profile: ' + error.message, 'error')
        console.error(error)
      } else {
        showToast('Error exporting profile: Unknown error', 'error')
        console.error(error)
      }
    }
  }, [profileData, currentProfileName])

  // Keyboard shortcuts
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.altKey && e.key === 'n') {
        e.preventDefault()
        handleNewProfile()
      }

      if (e.altKey && e.key === 'l') {
        e.preventDefault()
        if (isModified) {
          if (
            !confirm(
              'You have unsaved changes. Are you sure you want to load a different profile?',
            )
          ) {
            return
          }
        }
        setShowLoadModal(true)
      }

      if (e.altKey && e.key === 'i') {
        e.preventDefault()
        if (isModified) {
          if (
            !confirm(
              'You have unsaved changes. Are you sure you want to import a new profile?',
            )
          ) {
            return
          }
        }
        document.getElementById('file-input')?.click()
      }

      if (e.altKey && e.key === 'e') {
        e.preventDefault()
        handleExportProfile()
      }

      if (e.ctrlKey && e.key === 's') {
        e.preventDefault()
        if (isModified) {
          handleSaveProfile()
        }
      }

      if (e.ctrlKey && e.key === 'ArrowLeft') {
        e.preventDefault()
        const currentIndex = tabs.findIndex((tab) => tab.id === activeTab)
        if (currentIndex > 0) {
          setActiveTab(tabs[currentIndex - 1].id)
        }
      }

      if (e.ctrlKey && e.key === 'ArrowRight') {
        e.preventDefault()
        const currentIndex = tabs.findIndex((tab) => tab.id === activeTab)
        if (currentIndex < tabs.length - 1) {
          setActiveTab(tabs[currentIndex + 1].id)
        }
      }
    }
    window.addEventListener('keydown', handleKeyDown)
    return () => window.removeEventListener('keydown', handleKeyDown)
  }, [
    activeTab,
    isModified,
    handleNewProfile,
    handleSaveProfile,
    handleExportProfile,
    setActiveTab,
  ])

  // JSON import only: xlsx import (unified multipart /validate +
  // /parse-xlsx) is deferred until #524's unified /validate contract
  // merges.
  const handleImportFile = async (file: File) => {
    try {
      const fileExtension = file.name.split('.').pop()?.toLowerCase()
      const isJSON = fileExtension === 'json'

      if (!isJSON) {
        showToast('Please select a valid JSON (.json) file', 'warning')
        return
      }

      let baseFileName = file.name.replace(/\.json$/i, '')
      // Applied to the panel only once the profile installs into state
      // below. Empty means clean.
      let importValidationErrors: ValidationError[] = []

      const text = await file.text()
      const parsedProfile = JSON.parse(text)
      const { error } = await validateProfile({
        body: parsedProfile as PicsProfile,
      })
      const validateOutcome = interpretValidateError(error)

      // Cast is okay: the shape matches PicsProfile regardless of whether
      // it is semantically valid. A profile that fails validation still
      // loads, with its errors shown in the panels instead of rejected.
      const loadedData = parsedProfile as PicsProfile
      switch (validateOutcome.kind) {
        case 'clean':
          importValidationErrors = []
          break
        case 'invalid':
          importValidationErrors = validateOutcome.errors
          break
        case 'unconfirmed':
          // Non-structured error (e.g. a transport failure): still load,
          // but say plainly that we could not confirm validity rather
          // than silently reporting clean.
          importValidationErrors = [VALIDATION_UNCONFIRMED_ERROR]
          break
      }

      const { error: checkError } = await getProfile({
        baseUrl: '',
        throwOnError: false,
        path: { name: baseFileName },
      })
      const fileExists = !checkError

      if (fileExists) {
        const overwrite = confirm(
          `A profile named "${baseFileName}.json" already exists on the server.\n\n` +
            `Do you want to overwrite it?\n\n` +
            `Click "OK" to overwrite, or "Cancel" to save with a different name.`,
        )

        if (!overwrite) {
          const newName = prompt(
            'Enter a new name for this profile:',
            baseFileName,
          )
          if (!newName || newName.trim() === '') {
            // Nothing saves or loads, but the errors already computed from
            // a real server response must not vanish with the cancel.
            if (importValidationErrors.length > 0) {
              setImportFailureErrors(importValidationErrors)
            }
            return
          }
          baseFileName = newName.trim().replace(/[^a-zA-Z0-9_-]/g, '_')
        }
      } else {
        const saveName = prompt(
          'Enter a name to save this profile on the server:',
          baseFileName,
        )
        if (!saveName || saveName.trim() === '') {
          // Same rationale as the cancelled-rename branch above: don't
          // let the cancel swallow errors already known from /validate.
          if (importValidationErrors.length > 0) {
            setImportFailureErrors(importValidationErrors)
          }
          return
        }
        baseFileName = saveName.trim().replace(/[^a-zA-Z0-9_-]/g, '_')
      }

      const { error: saveError } = await saveProfile({
        baseUrl: '',
        throwOnError: false,
        body: {
          name: baseFileName,
          profile: loadedData,
        },
      })

      if (saveError) {
        // Parsing may have succeeded but nothing reached the server, so
        // don't swap the on-screen profile for data that was never saved.
        // Same "no profile" popup path as a parse failure; the save-failure
        // notice leads, the parsed errors (if any) follow.
        setImportFailureErrors([
          {
            point: '',
            message:
              `Profile parsed successfully but could not be saved to the ` +
              `server as "${baseFileName}.json". Fix the issues below, if ` +
              `any, and try importing again.`,
          },
          ...importValidationErrors,
        ])
        return
      }

      setProfileData(loadedData)
      setValidationErrors(importValidationErrors)
      markProfileAsSaved(baseFileName, loadedData)
      showToast(`Profile loaded and saved as "${baseFileName}.json"`)
    } catch (error: unknown) {
      if (error instanceof Error) {
        showToast('Error loading file: ' + error.message, 'error')
        console.error(error)
      } else {
        showToast('Error loading file: Unknown error', 'error')
      }
    }
  }

  const handleLoadFromServer = () => {
    if (isModified) {
      if (
        !confirm(
          'You have unsaved changes. Are you sure you want to load a different profile?',
        )
      ) {
        return
      }
    }
    setShowLoadModal(true)
  }

  // Resolve a profile by name through the SDK and, if successful, install it
  // in app state.
  const loadProfileByName = useCallback(
    async (
      profileName: string,
      notify = true,
    ): Promise<ProfileSource | null> => {
      const { data: profileJson, error: getError } = await getProfile({
        baseUrl: '',
        throwOnError: false,
        path: { name: profileName },
      })

      if (getError || !profileJson) {
        if (notify) {
          showToast(
            `Error loading profile "${profileName}": ${getError ? String(getError) : 'unknown error'}`,
            'error',
          )
        }
        return null
      }

      const { data: list } = await listProfiles({
        baseUrl: '',
        throwOnError: false,
      })
      const entry = list?.find((item) => item.name === profileName)
      const source: ProfileSource = entry?.source ?? 'seed'

      // `GET /api/profiles/:name` serves the file with no validation of
      // its own, so the load path calls `/validate` itself and installs
      // whatever it finds alongside the profile, same as the import path.
      const { error: validateError } = await validateProfile({
        body: profileJson,
      })
      const validateOutcome = interpretValidateError(validateError)
      let loadValidationErrors: ValidationError[]
      switch (validateOutcome.kind) {
        case 'clean':
          loadValidationErrors = []
          break
        case 'invalid':
          loadValidationErrors = validateOutcome.errors
          break
        case 'unconfirmed':
          // Same as the import path: a failed /validate request must
          // never read as "profile is clean".
          loadValidationErrors = [VALIDATION_UNCONFIRMED_ERROR]
          break
      }

      setProfileData(profileJson)
      setValidationErrors(loadValidationErrors)
      setOriginalProfileData(structuredClone(profileJson))
      setCurrentProfileName(profileName)
      setLoadedSource(source)
      setIsModified(false)

      if (notify) {
        showToast(`Profile "${profileName}" loaded successfully!`)
      }
      return source
    },
    [],
  )

  const handleSelectProfile = async (profileName: string): Promise<boolean> => {
    const source = await loadProfileByName(profileName, true)
    return source !== null
  }

  // Bootstrap.
  useEffect(() => {
    const lastName = localStorage.getItem(LAST_LOADED_KEY)
    const candidates = [lastName, DEFAULT_PROFILE_NAME].filter(
      (n): n is string => typeof n === 'string' && n.length > 0,
    )
    const unique = Array.from(new Set(candidates))

    let cancelled = false
    void (async () => {
      for (const name of unique) {
        if (cancelled) return
        const source = await loadProfileByName(name, false)
        if (source !== null) return
        if (name === lastName && name !== DEFAULT_PROFILE_NAME) {
          localStorage.removeItem(LAST_LOADED_KEY)
        }
      }
      if (!cancelled) {
        showToast(
          `Could not load default profile "${DEFAULT_PROFILE_NAME}". Use Load Profile to pick one.`,
          'warning',
        )
      }
    })()
    return () => {
      cancelled = true
    }
  }, [loadProfileByName])

  const handleCopyProfile = async () => {
    if (!profileData) {
      showToast('No profile data to copy!', 'warning')
      return
    }

    try {
      const defaultName = currentProfileName
        ? `${currentProfileName}_copy`
        : 'profile_copy'
      const newName = prompt(
        'Enter a new name for this profile copy:',
        defaultName,
      )

      if (!newName || newName.trim() === '') {
        return
      }

      const sanitizedName = newName.trim().replace(/[^a-zA-Z0-9_-]/g, '_')

      const snapshot = profileData
      const { error: copyError } = await saveProfile({
        baseUrl: '',
        throwOnError: false,
        body: {
          name: sanitizedName,
          profile: snapshot,
        },
      })

      if (copyError) {
        throw new Error('Failed to copy profile to server')
      }

      markProfileAsSaved(sanitizedName, snapshot)
      showToast(`Profile copied as "${sanitizedName}.json"`)
    } catch (error: unknown) {
      if (error instanceof Error) {
        showToast('Error copying profile: ' + error.message, 'error')
        console.error(error)
      } else {
        showToast('Error copying profile: Unknown error', 'error')
      }
    }
  }

  const handleEntityChange = (
    entityType: EquipmentGroup,
    newValue: number | string,
  ): boolean => {
    const value = typeof newValue === 'string' ? parseInt(newValue) : newValue

    if (!Number.isInteger(value) || value < 0) {
      showToast('Please enter a valid whole number (0 or greater)', 'warning')
      return false
    }

    if (!profileData) {
      showToast('Profile data is not loaded', 'error')
      return false
    }

    const oldValue = getEquipmentCount(profileData, entityType)

    if (value < oldValue) {
      const warning = `Warning: Decreasing ${entityType} from ${oldValue} to ${value} will remove ${oldValue - value} ${entityType} and their associated points. Continue?`
      if (!confirm(warning)) {
        return false
      }
    }

    setProfileData(setEquipmentCount(profileData, entityType, value))
    return true
  }

  const handlePointUpdate = (
    point: FlatPoint,
    field:
      | 'name'
      | 'mandatory_1815'
      | 'mandatory_1547'
      | 'value'
      | 'purpose'
      | 'units'
      | 'iec_61850_uid',
    value: string | number | boolean | null,
  ) => {
    if (!profileData) {
      showToast('Profile data is not loaded', 'error')
      return
    }

    try {
      // The locator is the address into the canonical sub-struct; updatePoint
      // returns a fresh profile with the named field rewritten.
      setProfileData(updatePoint(profileData, point.locator, field, value))
    } catch (error: unknown) {
      if (error instanceof Error) {
        showToast(error.message, 'error')
      } else {
        showToast('An unknown error occurred', 'error')
      }
    }
  }

  const handlePreferenceChange = (
    key: keyof Preferences,
    value: Preferences[keyof Preferences],
  ) => {
    setPreferences((prev) => updatePreference(prev, key, value))
  }

  // Re-runs validation against the CURRENT edited profile. A transport or
  // unexpected-shape failure is a third outcome, distinct from valid and
  // invalid: it must not clear the panel to a false clean state.
  const handleValidateProfile = useCallback(async () => {
    if (!profileData) {
      showToast('No profile loaded to validate', 'warning')
      return
    }

    const { error } = await validateProfile({ body: profileData })
    const outcome = interpretValidateError(error)

    switch (outcome.kind) {
      case 'clean':
        setValidationErrors([])
        showToast('Profile is valid.', 'success')
        return
      case 'invalid':
        setValidationErrors(outcome.errors)
        showToast(
          `Validation found ${outcome.errors.length} issue(s).`,
          'warning',
        )
        return
      case 'unconfirmed':
        // Panel left as-is: a failed re-check of an already-displayed
        // profile shouldn't erase what was already known about it.
        showToast(
          'Validate request failed; the panel was left unchanged.',
          'error',
        )
        return
    }
  }, [profileData])

  return (
    <>
      <div className="max-w-[1600px] mx-auto p-3 bg-white min-h-screen">
        <Header
          onImportFile={handleImportFile}
          onLoadFromServer={handleLoadFromServer}
          onSave={handleSaveProfile}
          onExport={handleExportProfile}
          onCopy={handleCopyProfile}
          onNewProfile={handleNewProfile}
          onOpenPreferences={() => setShowPreferencesModal(true)}
          onValidate={handleValidateProfile}
          profileName={currentProfileName}
          isModified={isModified}
        />

        <Tabs
          tabs={tabs}
          activeTab={activeTab}
          onTabChange={setActiveTab}
          errorCounts={tabErrorCounts}
        />

        {/* Errors that don't resolve to a single tab (structure/workbook
            faults). Sits above the tab system so it's always visible. */}
        <ValidationErrorCallout
          errors={validationBuckets.general}
          title="Profile validation issues"
        />

        <div>
          {/* Test Runner tab is always available */}
          {isTabMounted('test_runner') && profileData && (
            <div
              style={{
                display: activeTab === 'test_runner' ? 'block' : 'none',
              }}
            >
              <TestRunnerTab
                profileData={profileData}
                preferences={preferences}
                currentProfileName={currentProfileName}
              />
            </div>
          )}

          {/* Other tabs require profile data */}
          {!profileData && (
            <div className="flex flex-col items-center justify-center py-15 text-muted-foreground">
              <h3 className="text-lg font-semibold">No Data Available</h3>
              <p className="text-sm">Load a profile to begin editing.</p>
            </div>
          )}

          {profileData && (
            <>
              {isTabMounted('entities') && (
                <div
                  style={{
                    display: activeTab === 'entities' ? 'block' : 'none',
                  }}
                >
                  <EntitiesTab
                    onEntityChange={handleEntityChange}
                    profileData={profileData}
                    setProfileData={setProfileData}
                  />
                </div>
              )}

              {isTabMounted('scheduling') && (
                <div
                  style={{
                    display: activeTab === 'scheduling' ? 'block' : 'none',
                  }}
                >
                  <SchedulingTab
                    profileData={profileData}
                    setProfileData={setProfileData}
                    errors={validationBuckets.scheduling}
                  />
                </div>
              )}

              {isTabMounted('curves') && (
                <div
                  style={{ display: activeTab === 'curves' ? 'block' : 'none' }}
                >
                  <EnumDataProvider>
                    <CurvesTab
                      profileData={profileData}
                      setProfileData={setProfileData}
                      errors={validationBuckets.curves}
                    />
                  </EnumDataProvider>
                </div>
              )}

              {tabs.map(
                (tab) =>
                  isTabMounted(tab.id) &&
                  isPointSection(tab.id) &&
                  activeTab === tab.id && (
                    <div key={tab.id}>
                      <PointsTab
                        tabName={tab.id}
                        profileData={profileData}
                        onPointUpdate={handlePointUpdate}
                        purposeFilter={purposeFilter}
                        onPurposeFilterChange={setPurposeFilter}
                        autoExpandSections={preferences.autoExpandSections}
                        errors={validationBuckets[tab.id]}
                      />
                    </div>
                  ),
              )}
            </>
          )}
        </div>
      </div>

      {/* Load Profile Modal */}
      <ProfileListModal
        isOpen={showLoadModal}
        onClose={() => setShowLoadModal(false)}
        onSelect={handleSelectProfile}
        mode="load"
      />

      {/* Preferences Modal */}
      <PreferencesModal
        isOpen={showPreferencesModal}
        onClose={() => setShowPreferencesModal(false)}
        preferences={preferences}
        onPreferenceChange={handlePreferenceChange}
      />

      {/* Save-as prompt for seed-sourced (or unnamed) profiles. */}
      <SaveAsModal
        isOpen={showSaveAsModal}
        defaultName={currentProfileName ?? ''}
        fromSeed={loadedSource === 'seed'}
        onClose={() => setShowSaveAsModal(false)}
        onConfirm={handleSaveAsConfirm}
      />

      {/* Import that produced no profile: dismissible popup, disjoint from
          the panel above. */}
      <ImportErrorDialog
        open={importFailureErrors !== null}
        errors={importFailureErrors ?? []}
        onClose={() => setImportFailureErrors(null)}
      />
    </>
  )
}

export default App
