export type DescriptionSegment =
  | { kind: 'text'; text: string }
  | { kind: 'entry'; value: string; label: string }

export interface ParsedPointDescription {
  segments: DescriptionSegment[]
  entryCount: number
}

/** Presentation only: never normalize or write back the profile description. */
export function parsePointDescription(text: string): ParsedPointDescription {
  // Include unsupported numeric tokens as boundaries so their text cannot be
  // swallowed by the preceding definition. Match entities, not arbitrary HTML.
  const markers = [
    ...text.matchAll(/<(\d[^<>\n]*)>|&lt;(\d(?:(?!&gt;)[^\n])*?)&gt;/g),
  ]
  const entries: {
    start: number
    end: number
    value: string
    label: string
  }[] = []

  markers.forEach((marker, index) => {
    const value = marker[1] ?? marker[2]
    if (!/^\d+(?:-\d+)?$/.test(value)) return
    const start = marker.index
    if (start > 0 && !/\s/.test(text[start - 1])) return
    const labelStart = start + marker[0].length
    const following = text.slice(
      labelStart,
      markers[index + 1]?.index ?? text.length,
    )
    // A definition has a separated label, not just a prose reference such as
    // "Default is <3>.". Do not consume the next line as an empty token's label.
    if (!/^[\t ]+\S/.test(following)) return
    const boundary = following.search(/\r?\n|\.{3}|…/)
    const labelText = boundary < 0 ? following : following.slice(0, boundary)
    const label = labelText.trim()
    if (!/^[\p{L}\p{N}]/u.test(label)) return
    entries.push({
      start,
      end: labelStart + labelText.trimEnd().length,
      value,
      label,
    })
  })

  // A lone inline bracketed number is more likely prose than a mapping unless
  // the source explicitly identifies it as an enumeration or starts a line.
  const confident =
    entries.length > 1 ||
    (entries.length === 1 &&
      (/Enumeration\s*:/i.test(text.slice(0, entries[0].start)) ||
        /(?:^|\n)[\t ]*$/.test(text.slice(0, entries[0].start))))
  if (!confident) return { segments: [{ kind: 'text', text }], entryCount: 0 }

  const segments: DescriptionSegment[] = []
  let cursor = 0
  for (const entry of entries) {
    const prose = text.slice(cursor, entry.start)
    if (prose.trim()) segments.push({ kind: 'text', text: prose })
    segments.push({ kind: 'entry', value: entry.value, label: entry.label })
    cursor = entry.end
  }
  const trailing = text.slice(cursor)
  if (trailing.trim()) segments.push({ kind: 'text', text: trailing })
  return { segments, entryCount: entries.length }
}
