import { useEffect, useRef, useState } from 'react'
import { toast } from 'sonner'
import {
  addProfileCurve,
  resizeProfileCurvePoints,
  type PicsProfile,
} from '@/api/generated'
import { isValidationErrorsResponse } from './importResponse'

interface CurveMutationOptions {
  profile: PicsProfile | null
  setProfile: (profile: PicsProfile) => void
  onCurvesAdded: () => void
}

/** Backend-backed curve edits shared by the Entities and Curves tabs. */
export function useCurveMutations({
  profile,
  setProfile,
  onCurvesAdded,
}: CurveMutationOptions) {
  const currentProfileRef = useRef(profile)
  useEffect(() => {
    currentProfileRef.current = profile
  }, [profile])
  const resizePending = useRef(false)
  const [isResizingCurvePoints, setIsResizingCurvePoints] = useState(false)

  const addCurves = async (count = 1): Promise<number | null> => {
    if (!profile) return null
    try {
      const { data, error } = await addProfileCurve({
        body: { profile, count },
      })
      if (!data) {
        throw new Error(
          isValidationErrorsResponse(error)
            ? error.errors.errors.map((entry) => entry.message).join('\n')
            : 'Failed to add curves.',
        )
      }
      if (currentProfileRef.current !== profile) {
        throw new Error(
          'The profile changed while adding curves. Please retry.',
        )
      }
      setProfile(data.profile)
      onCurvesAdded()
      return data.added_curve_index
    } catch (error) {
      toast.error(
        error instanceof Error ? error.message : 'Failed to add curves.',
      )
      return null
    }
  }

  const resizeCurvePoints = async (
    curveIndex: number,
    count: number,
  ): Promise<void> => {
    if (!profile || resizePending.current) return
    resizePending.current = true
    setIsResizingCurvePoints(true)
    try {
      const { data, error } = await resizeProfileCurvePoints({
        body: { profile, curve_index: curveIndex, count },
      })
      if (!data) {
        throw new Error(
          isValidationErrorsResponse(error)
            ? error.errors.errors.map((entry) => entry.message).join('\n')
            : 'Failed to resize curve points.',
        )
      }
      if (currentProfileRef.current !== profile) {
        throw new Error(
          'The profile changed while resizing curve points. Please retry.',
        )
      }
      setProfile(data.profile)
      // Resizing validates only the operation, not the whole profile.
    } catch (error) {
      toast.error(
        error instanceof Error
          ? error.message
          : 'Failed to resize curve points.',
      )
    } finally {
      resizePending.current = false
      setIsResizingCurvePoints(false)
    }
  }

  return { addCurves, resizeCurvePoints, isResizingCurvePoints }
}
