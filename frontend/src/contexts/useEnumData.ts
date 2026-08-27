import { useContext } from 'react'
import { createContext } from 'react'
import type { EnumsResponse } from '@/api/generated'

export type EnumDataContextValue = {
  enumsResponse: EnumsResponse
  isLoading: boolean
  error: Error | null
}

export function useEnumData() {
  const context = useContext(EnumDataContext)

  if (!context) {
    throw new Error('useEnumData must be used within an EnumDataProvider.')
  }

  return context
}

export const EnumDataContext = createContext<EnumDataContextValue | null>(null)
