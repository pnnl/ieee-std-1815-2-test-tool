import {
  parsePointDescription,
  type DescriptionSegment,
} from '@/utils/parsePointDescription'

function DescriptionContent({ segments }: { segments: DescriptionSegment[] }) {
  return (
    <div className="min-w-0 space-y-1 text-xs">
      {segments.map((segment, index) =>
        segment.kind === 'entry' ? (
          <dl
            key={index}
            className="grid min-w-0 grid-cols-[3.5rem_minmax(0,1fr)] gap-x-2"
          >
            <dt className="whitespace-nowrap font-mono text-muted-foreground">
              {segment.value}
            </dt>
            <dd className="min-w-0 whitespace-pre-wrap break-words">
              {segment.label}
            </dd>
          </dl>
        ) : (
          <div
            key={index}
            className="col-span-2 whitespace-pre-wrap break-words"
          >
            {segment.text.trim()}
          </div>
        ),
      )}
    </div>
  )
}

export default function PointDescription({ text }: { text: string }) {
  const { segments, entryCount } = parsePointDescription(text)
  if (!entryCount) return <>{text}</>
  const firstEntry = segments.findIndex((segment) => segment.kind === 'entry')
  const introduction = segments.slice(0, firstEntry)
  const definitions = segments.slice(firstEntry)

  return (
    <div className="min-w-0 space-y-2">
      {introduction.map((segment, index) =>
        segment.kind === 'text' ? (
          <p key={index} className="whitespace-pre-wrap break-words">
            {segment.text.trim()}
          </p>
        ) : null,
      )}
      {entryCount > 8 ? (
        <details className="min-w-0">
          <summary className="cursor-pointer rounded-sm text-xs font-medium focus-visible:outline-2 focus-visible:outline-ring">
            Enumeration ({entryCount} entries)
          </summary>
          <div className="mt-2">
            <DescriptionContent segments={definitions} />
          </div>
        </details>
      ) : (
        <DescriptionContent segments={definitions} />
      )}
    </div>
  )
}
