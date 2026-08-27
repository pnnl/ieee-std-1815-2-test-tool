import { Tabs as ShadcnTabs, TabsList, TabsTrigger } from '@/components/ui/tabs'
import ValidationErrorBadge from '@/components/ValidationErrorBadge'

interface Tab {
  id: string
  label: string
}

interface TabsProps {
  tabs: Tab[]
  activeTab: string
  onTabChange: (tabId: string) => void
  // Per-tab validation error counts, keyed by tab id. A tab with no entry,
  // or a count of 0, renders no badge.
  errorCounts?: Readonly<Record<string, number>>
}

function Tabs({ tabs, activeTab, onTabChange, errorCounts }: TabsProps) {
  return (
    <ShadcnTabs value={activeTab} onValueChange={onTabChange}>
      <TabsList>
        {tabs.map((tab) => {
          const count = errorCounts?.[tab.id] ?? 0
          return (
            <TabsTrigger key={tab.id} value={tab.id}>
              {tab.label}
              {count > 0 && <ValidationErrorBadge count={count} />}
            </TabsTrigger>
          )
        })}
      </TabsList>
    </ShadcnTabs>
  )
}

export default Tabs
