import { useCallback, useEffect, useState, type ReactNode } from 'react'
import { getEnums, type EnumsResponse } from '@/api/generated'
import { EnumDataContext } from './useEnumData'
import { Spinner } from '@radix-ui/themes'

type EnumDataProviderProps = {
  children: ReactNode
}

export function EnumDataProvider({ children }: EnumDataProviderProps) {
  const [enumsResponse, setData] = useState<EnumsResponse | null>(null)
  const [isLoading, setIsLoading] = useState(true)
  const [error, setError] = useState<Error | null>(null)

  const reload = useCallback(async () => {
    setIsLoading(true)
    setError(null)

    try {
      const { data: enumData, error: enumError } = await getEnums()

      if (enumError || !enumData) {
        throw new Error('Failed to load enum data.')
      }

      setData(enumData)
    } catch (err) {
      setError(
        err instanceof Error ? err : new Error('Failed to load enum data.'),
      )
    } finally {
      setIsLoading(false)
    }
  }, [])

  useEffect(() => {
    void reload()
  }, [reload])

  if (!enumsResponse) {
    return <Spinner className="m-4" />
  }

  return (
    <EnumDataContext.Provider value={{ enumsResponse, isLoading, error }}>
      {children}
    </EnumDataContext.Provider>
  )
}
