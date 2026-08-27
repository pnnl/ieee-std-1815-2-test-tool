# Data Directory Structure

This directory contains the template profile, curated seed profiles, and user-created profiles for the MESA Profile Editor.

## Directory Structure

```
data/
├── template/
│   └── profile.json      # Template profile (READ-ONLY)
├── profiles/
│   ├── full.json         # Curated seed profiles (READ-ONLY)
│   ├── demo.json
│   └── ...
└── working/
    ├── .gitkeep          # Tracked, keeps the directory in git
    ├── my_profile.json   # User-saved profiles (gitignored)
    └── ...
```

## Template Directory

**Location:** `data/template/`

**Purpose:** Contains the template profile used as the basis for creating new profiles.

**Files:**
- `profile.json` - The default IEEE 1815.2 PICS profile template

**Important:**
- This directory is mounted **read-only** in Docker containers
- The template should **never** be modified by the application
- Users cannot accidentally overwrite this file through the UI

## Profiles Directory

**Location:** `data/profiles/`

**Purpose:** Stores curated seed profiles distributed with the application (for example `full.json`, `demo.json`, `mandatory_*`, `minimal_*`).

**Important:**
- This directory is mounted **read-only** in Docker containers
- Seed profiles are version-controlled and should be edited through pull requests, not at runtime
- The `/api/profiles` endpoints surface these entries with `source: "seed"`

## Working Directory

**Location:** `data/working/`

**Purpose:** Stores all user-created and modified profiles at runtime.

**Files:**
- User profiles are saved as `{name}.json`
- Profiles created from Excel files
- Profiles copied from the template, a seed profile, or another working profile

**Important:**
- This directory is mounted **read-write** in Docker containers
- All saves from the UI and API land here; `data/profiles/` is never written
- The `/api/profiles` endpoints surface these entries with `source: "working"`
- Profile names are sanitized to contain only alphanumeric characters, hyphens, and underscores
- `data/working/.gitkeep` is committed; everything else under `data/working/` is gitignored

## Resolution Order

The `/api/profiles` listing aggregates both directories. When the same profile name exists in both, the entry in `data/working/` shadows the one in `data/profiles/`, so a user save with the same filename as a seed profile takes precedence at read time.

## Setup Instructions

### Development

1. Create the directory structure:
   ```bash
   mkdir -p data/template
   mkdir -p data/profiles
   mkdir -p data/working
   ```

2. Place your template profile:
   ```bash
   # Copy your template profile to the template directory
   cp profile.json data/template/profile.json
   ```

3. Start the development environment:
   ```bash
   docker compose -f docker-compose.dev.yml up -d
   ```

### Production

1. Ensure the directory structure exists (same as development)

2. Start the production environment:
   ```bash
   docker compose up -d
   ```

## File Permissions

- `data/template/` - Should be readable by the web server
- `data/profiles/` - Should be readable by the web server
- `data/working/` - Should be readable and writable by the web server

## Backup Recommendations

- **Template**: Version control `data/template/profile.json` as it's the source of truth
- **Seed Profiles**: Tracked in git under `data/profiles/`; treat changes as code review
- **User Profiles**: Regularly backup `data/working/` to prevent data loss; this directory is not version-controlled

## Notes

- The template profile at `data/template/profile.json` is loaded when:
  - User clicks "New Profile"
- User profiles are saved to `data/working/` when:
  - User loads an Excel or JSON file
  - User clicks "Save" to save current modifications
  - User clicks "Copy" to duplicate a profile with a new name
  - User creates a new profile from template
