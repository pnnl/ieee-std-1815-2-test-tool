import { useState } from 'react'
import { NumberInput } from '@/components/ui/number-input'
import { Alert, AlertTitle, AlertDescription } from '@/components/ui/alert'
import { MAX_CURVES } from '../utils/curveUtils'
import {
  getEquipmentCount,
  setCurveCount,
  setScheduleCount,
  type EquipmentGroup,
} from '@/profile/canonical'
import type { PicsProfile } from '@/api/generated'

interface EntitiesTabProps {
  onEntityChange: (entityType: EquipmentGroup, value: string) => boolean
  profileData: PicsProfile
  setProfileData: (profile: PicsProfile) => void
}

const ENTITY_FIELDS: {
  key: EquipmentGroup
  label: string
  description: string
}[] = [
  { key: 'meters', label: 'Meters', description: 'Number of metering devices' },
  {
    key: 'ders',
    label: 'DER Units',
    description: 'Distributed energy resources',
  },
  {
    key: 'inverters',
    label: 'Inverters',
    description: 'Power inverter devices',
  },
  {
    key: 'batteries',
    label: 'Batteries',
    description: 'Battery storage units',
  },
]

function EntitiesTab({
  onEntityChange,
  profileData,
  setProfileData,
}: EntitiesTabProps) {
  const [warning, setWarning] = useState('')

  const curveCount = profileData.AI.curves.length
  const scheduleCount = profileData.AI.schedules.length

  const handleChange = (entityType: EquipmentGroup, value: string | number) => {
    const strValue = String(value)
    const oldValue = getEquipmentCount(profileData, entityType)
    const newValue = parseInt(strValue)

    const success = onEntityChange(entityType, strValue)
    if (!success) return

    if (newValue < oldValue) {
      setWarning(
        `${entityType.charAt(0).toUpperCase() + entityType.slice(1)} count decreased from ${oldValue} to ${newValue}. Associated points have been removed.`,
      )
    } else {
      setWarning('')
    }
  }

  const handleCurveCountChange = (value: string | number) => {
    const newCount = Math.max(0, Math.min(MAX_CURVES, Number(value) || 0))
    setProfileData(setCurveCount(profileData, newCount))
  }

  const handleScheduleCountChange = (value: string | number) => {
    const newCount = Math.max(0, Math.min(100, Number(value) || 0))
    setProfileData(setScheduleCount(profileData, newCount))
  }

  return (
    <div className="flex flex-col gap-6 p-6 max-w-[900px]">
      <div>
        <h3 className="text-lg font-semibold mb-1">Entity Configuration</h3>
        <p className="text-sm text-muted-foreground">
          Configure the number of each entity type in your IEEE 1815.2 profile.
        </p>
      </div>

      <Alert className="border-warning/50 bg-warning/10">
        <AlertTitle className="text-warning font-medium">Caution</AlertTitle>
        <AlertDescription>
          Decreasing entity counts will permanently remove associated points
          from the profile.
        </AlertDescription>
      </Alert>

      <div className="grid grid-cols-1 sm:grid-cols-2 md:grid-cols-3 gap-4">
        {ENTITY_FIELDS.map(({ key, label, description }) => (
          <div key={key} className="border rounded-md p-4">
            <p className="font-semibold text-sm mb-0.5">{label}</p>
            <p className="text-muted-foreground text-xs mb-3">{description}</p>
            <NumberInput
              min={0}
              step={1}
              value={getEquipmentCount(profileData, key)}
              onChange={(val) => handleChange(key, String(val))}
              styles={{ input: { fontWeight: 600, fontSize: 16 } }}
            />
          </div>
        ))}
        <div className="border rounded-md p-4">
          <p className="font-semibold text-sm mb-0.5">Curves</p>
          <p className="text-muted-foreground text-xs mb-3">
            Number of curve definitions
          </p>
          <NumberInput
            min={0}
            max={MAX_CURVES}
            step={1}
            value={curveCount}
            onChange={(val) => handleCurveCountChange(val)}
            styles={{ input: { fontWeight: 600, fontSize: 16 } }}
          />
        </div>
        <div className="border rounded-md p-4">
          <p className="font-semibold text-sm mb-0.5">Schedules</p>
          <p className="text-muted-foreground text-xs mb-3">
            Number of schedule entries
          </p>
          <NumberInput
            min={0}
            max={100}
            step={1}
            value={scheduleCount}
            onChange={(val) => handleScheduleCountChange(val)}
            styles={{ input: { fontWeight: 600, fontSize: 16 } }}
          />
        </div>
      </div>

      {warning && (
        <Alert className="border-warning/50 bg-warning/10">
          <AlertDescription className="text-warning">
            {warning}
          </AlertDescription>
        </Alert>
      )}
    </div>
  )
}

export default EntitiesTab
