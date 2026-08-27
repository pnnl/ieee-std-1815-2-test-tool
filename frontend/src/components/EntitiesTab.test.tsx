// EntitiesTab reads profileData.AI.curves; a profile missing the AI key
// makes that undefined and throws. This documents the CURRENT throw
// behavior; when a shape mismatch instead surfaces as a rendered
// General-panel error, update this test to assert that message instead.
//
// `GET /api/profiles/:name` serves seed files as raw JSON with no shape
// validation (#492), so a profile record missing a required key can reach
// this component as-is. The fixture below is derived from full.json, the
// known-good PicsProfile, by omitting AI: a deviation from the generated
// type rather than a hand-written non-canonical blob.

import { describe, it, expect } from 'vitest'
import { screen } from '@testing-library/react'
import EntitiesTab from './EntitiesTab'
import type { PicsProfile } from '@/api/generated'
import fullProfileJson from '../../../data/profiles/full.json'
import { renderWithTheme } from '@/testUtils'

describe('EntitiesTab with a profile missing the AI key', () => {
  it('throws when AI is missing from an otherwise-valid PicsProfile (#492: documents current behavior)', () => {
    const { AO, BI, BO, CTR, Key } = fullProfileJson as unknown as PicsProfile
    const profileMissingAi = { AO, BI, BO, CTR, Key } as unknown as PicsProfile

    expect(() =>
      renderWithTheme(
        <EntitiesTab
          onEntityChange={() => true}
          profileData={profileMissingAi}
          setProfileData={() => {}}
        />,
      ),
    ).toThrow(/Cannot read properties of undefined/)
  })

  it('still renders full.json, the known-good reference fixture, unaffected by the missing-AI case', () => {
    const fullProfile = fullProfileJson as unknown as PicsProfile

    expect(() =>
      renderWithTheme(
        <EntitiesTab
          onEntityChange={() => true}
          profileData={fullProfile}
          setProfileData={() => {}}
        />,
      ),
    ).not.toThrow()

    const spinbuttons = screen.getAllByRole('spinbutton')
    expect(spinbuttons).toHaveLength(6)
    // full.json's own real curve/schedule counts (per canonical.test.ts).
    expect((spinbuttons[4] as HTMLInputElement).value).toBe('4')
    expect((spinbuttons[5] as HTMLInputElement).value).toBe('4')
  })
})
