# reference_outstation

DNP3 Reference Outstation application. Listens for master connections over TCP/IP, serves point data, handles controls.

## Running

```sh
cargo run -- --local 0.0.0.0:20000 --profile ../../../data/profiles/full.json
```

CLI options:
- `--local <ADDR>` — TCP bind address (default: `0.0.0.0:20000`)
- `--outstation-address <ADDR>` — DNP3 address of this outstation (default: `1024`)
- `--master-address <ADDR>` — DNP3 address of the expected master / control station (default: `1`)
- `--profile <PATH>` — PICS profile JSON path (default: `data/template/profile.json`)
- `--log-level <LEVEL>` — log level: error, warn, info, debug, trace (default: `info`)

## Diagnosing connectivity issues

See the [reference_control_station README](../reference_control_station/README.md#diagnosing-connectivity-issues) for a full step-by-step guide.

The most common outstation-side issue is a DNP3 address mismatch: ensure `--outstation-address` and `--master-address` match the values configured on the connecting control station.
