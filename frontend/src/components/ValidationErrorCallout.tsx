// Renders a list of ValidationError entries in a Radix Themes Callout. Used
// both for the General panel (above the tabs) and for a single point tab's
// own bucket. Renders nothing when empty: an empty bucket must not leave a
// callout shell on screen.
//
// When a title is supplied (the General panel case), the count sits next to
// it as the same Badge used on the per-tab controls: General has no tab of
// its own to carry a badge, so it gets the same treatment on its title.

import { Callout } from '@radix-ui/themes'
import { AlertTriangle } from 'lucide-react'
import ValidationErrorBadge from './ValidationErrorBadge'
import type { ValidationError } from '@/api/generated'

interface ValidationErrorCalloutProps {
  errors: readonly ValidationError[]
  title?: string
}

function ValidationErrorCallout({
  errors,
  title,
}: ValidationErrorCalloutProps) {
  if (errors.length === 0) return null

  return (
    <Callout.Root color="red" className="mb-3">
      <Callout.Icon>
        <AlertTriangle size={16} aria-hidden="true" />
      </Callout.Icon>
      <Callout.Text>
        {title && (
          <div className="font-semibold mb-1 flex items-center gap-2">
            {/* `title` stays in its own node so exact text queries resolve
                to just the title, not the title plus the badge's text. */}
            <span>{title}</span>
            <ValidationErrorBadge count={errors.length} />
          </div>
        )}
        <ul className="list-disc pl-4 space-y-0.5">
          {errors.map((error, index) => (
            <li key={`${error.point}|${error.message}|${index}`}>
              {error.point ? (
                <span className="font-mono">{error.point}: </span>
              ) : null}
              {error.message}
            </li>
          ))}
        </ul>
      </Callout.Text>
    </Callout.Root>
  )
}

export default ValidationErrorCallout
