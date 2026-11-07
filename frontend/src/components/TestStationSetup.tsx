import { Badge } from '@/components/ui/badge'
import { isReferenceOutstation, type Preferences } from '@/utils/preferences'

interface TestStationSetupProps {
  preferences: Preferences
}

export default function TestStationSetup({
  preferences,
}: TestStationSetupProps) {
  const referenceOutstation = isReferenceOutstation(preferences.outstationIp)
  const host = referenceOutstation ? '127.0.0.1' : preferences.outstationIp

  return (
    <section
      aria-label="Test station setup"
      className="grid gap-3 sm:grid-cols-2"
    >
      <div className="rounded-md border bg-card p-3 space-y-2">
        <div className="flex flex-wrap items-center gap-2">
          <h3 className="text-sm font-semibold">Reference control station</h3>
          <Badge variant="secondary">Starts automatically</Badge>
        </div>
        <p className="text-sm text-muted-foreground">
          This DNP3 client starts locally for each test run. It connects to the
          outstation and runs the selected scenarios.
        </p>
      </div>
      <div className="rounded-md border bg-card p-3 space-y-2">
        <div className="flex flex-wrap items-center gap-2">
          <h3 className="text-sm font-semibold">
            {referenceOutstation
              ? 'Reference outstation'
              : 'External outstation'}
          </h3>
          <Badge variant="secondary">
            {referenceOutstation ? 'Starts automatically' : 'User-provided'}
          </Badge>
        </div>
        <p className="text-sm text-muted-foreground">
          {referenceOutstation
            ? 'The tool starts this DNP3 server locally using the loaded profile.'
            : 'Start your DNP3 server before running tests. The tool connects to this device; you manage it.'}
        </p>
        <p className="text-sm">
          Endpoint:{' '}
          <span className="font-mono break-all">
            {host}:{preferences.outstationPort}
          </span>
        </p>
      </div>
    </section>
  )
}
