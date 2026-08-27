# PICS Validator

Validates IEEE Std 1815.2-2025 companion data point tables for use in control station or outstation. Provides library loaders (`load_xlsx_profile`, `load_json_profile`) and a CLI that writes JSON output.

## Prerequisites

- When using XLSX input, the spreadsheet **must be opened and saved in Microsoft Excel** before validation to populate cached formula values.

## Input Files

| File | Purpose |
|------|---------|
| `data/pics.xlsx` | Standard reference spreadsheet (XLSX) |
| `data/pics.json` | Standard reference profile in JSON form |
| `data/profiles/*.xlsx` | Pre-built example profiles (full, mandatory_1547, mandatory_1815, minimal_1547) |

## Output

The CLI always writes JSON to the path you supply. The shape differs by input type:

| Input type | Output shape |
|------------|-------------|
| XLSX | `{ "profile": PicsProfile, "errors": [LoadError] }` |
| JSON | `PicsProfile` |

`LoadError` records field-level problems (missing or non-numeric values) that were skipped with a zero placeholder so the profile still loads.

## JSON format

The `PicsProfile` object schema:

```json
{
  "Key": { "...KeySheet fields..." },
  "BO": {
    "points": ["...BoPoint..."]
  },
  "BI": {
    "points":    ["...BiPoint..."],
    "meters":    ["...BiMeter..."],
    "ders":      ["...BiDer..."],
    "inverters": ["...BiInverter..."],
    "batteries": ["...BiBattery..."]
  },
  "AO": {
    "points":    ["...AoPoint..."],
    "meters":    ["...AoMeter..."],
    "inverters": ["...AoInverter..."],
    "batteries": ["...AoBattery..."]
  },
  "AI": {
    "points":       ["...AiPoint..."],
    "curves":       ["...AiCurve..."],
    "schedules_bc": ["...AiScheduleBC..."],
    "schedules":    ["...AiSchedule..."],
    "meters":       ["...AiMeter..."],
    "ders":         ["...AiDer..."],
    "inverters":    ["...AiInverter..."],
    "batteries":    ["...AiBattery..."]
  },
  "CTR": ["...CtrPoint..."]
}
```

Equipment groups (`meters`, `ders`, `inverters`, `batteries`) are only populated when the XLSX sheet contains the corresponding section header row. Profile files in `data/profiles/` are flat (no section headers) and load all points into the base `points` list.

## Build

```sh
cargo build --release
```

## Usage

```sh
# Load the reference PICS spreadsheet and export to JSON (includes validation errors)
cargo run -- data/pics.xlsx data/pics.json

# Load a pre-built profile spreadsheet
cargo run -- ../../../data/profiles/full.xlsx ../../../data/profiles/full.json

# Round-trip an existing JSON profile
cargo run -- data/pics.json data/pics_out.json
```
