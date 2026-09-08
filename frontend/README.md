# IEEE 1815.2 Test Tool Profile Editor - Frontend

A React-based web application for editing IEEE 1815.2 profile configuration files.

## Tech Stack

- **React 18** - UI framework
- **Vite** - Build tool and dev server
- **Vanilla CSS** - Styling (no CSS framework dependencies)

## Features

- **Entity Configuration**: Configure the number of meters, DERs, inverters, and batteries
  - Automatically adds/removes corresponding points when entity counts change
- **Point Editing**: Edit binary and analog inputs/outputs in Excel-like grids
- **Offset Organization**: Points are automatically grouped by their offset categories
- **Purpose Filter**: Filter points by purpose category across all tabs
- **Curves Editor**: Visual curve editor with drag-and-drop point manipulation
  - Support for 10 curves with 10 points each
  - Configure curve type, units, and point values
- **Preferences**: Configurable settings saved to browser localStorage
  - Control station and outstation IP/port settings
  - Auto-expand sections on tab change
- **File Operations**: Load existing profiles and save modified profiles
- **Validation**: Warns when decreasing entity counts that may affect data
- **Real-time Updates**: All changes are reflected immediately in the UI

## Getting Started

### Prerequisites

- Node.js 18 or higher
- npm or yarn

### Installation

```bash
cd frontend
npm install
npx playwright install-deps
npx playwright install
```

### Running the Development Server

```bash
npm run dev
```

The application will be available at http://localhost:3000

### Running with Docker Compose

The development stack uses Docker Compose with hot reload. From the repository
root, run:

```bash
make dev
```

Or invoke Docker Compose directly:

```bash
docker compose -f docker-compose.dev.yml up --build -d
```

The application will be available at http://localhost:3000.

To stop the stack, run `make dev-down` or
`docker compose -f docker-compose.dev.yml down`.

## Usage

### Creating a New Profile

1. Click the "New Profile" button in the toolbar
2. Enter a name for your new profile when prompted
3. The system will load the default profile template from the server
4. The new profile will be saved to the server and loaded in the editor
5. You can now edit the profile

**Note:** The default profile template is located at `/data/template/profile.json` on the server (read-only, never overwritten).

### Loading a Profile

1. Click the "Load Profile" button in the toolbar
2. Select either:
   - An **Excel file** (.xlsx or .xls) - will be parsed to JSON format
   - A **JSON profile file** (.json) - will be loaded directly
3. System checks if a profile with that name already exists on the server:
   - **If exists**: You'll be asked if you want to overwrite or save with a different name
   - **If not exists**: You'll be prompted to enter a name to save it on the server
4. The profile is saved to the server's `/data/profiles` directory
5. The profile is loaded and displayed in the editor across all tabs

**Example:** Loading `pics.xlsx` will:

- Parse the Excel file to JSON using the column mappings
- Check if `pics.json` exists on server
- Prompt for overwrite confirmation or new name
- Save to server as `{name}.json`
- Load into editor for editing

**Note:** Users must explicitly create or load a profile. The application does not auto-load a profile on startup.

### Editing Entities

1. Navigate to the "Entities" tab
2. Modify the count for meters, DERs, inverters, or batteries
3. Enter only whole numbers (integers)
4. If you decrease a count, you'll receive a warning about potential data loss

### Editing Points

1. Navigate to the desired tab (Binary Outputs, Binary Inputs, Analog Outputs, or Analog Inputs)
2. Points are organized by offset categories (headers show offset values)
3. Edit fields directly in the grid:
   - **Index**: Read-only identifier
   - **Description**: Point description (expandable text area)
   - **UID**: Unique identifier string
   - **Purpose**: Purpose category
   - **Value**: Numeric value
   - **Associated Index**: Related point reference
   - **Checkboxes**: IEEE standards compliance and support flags
   - **Units**: Units of measurement (analog points only)

### Saving Changes

1. Make edits to your profile (entities or points)
2. The title bar will show an asterisk (*) indicating unsaved changes
3. Click the "Save" button in the toolbar (only enabled when there are modifications)
4. The profile will be saved to the server's `/data/profiles` directory
5. The modification indicator (*) will be cleared

**Note:** The Save button overwrites the current profile. Use "Copy" to save as a new profile.

### Copying a Profile

1. Click "Copy" button in the toolbar
2. Enter a new name for the profile copy (defaults to `{current_name}_copy`)
3. The profile will be saved to the server's `/data/profiles` directory as `{new_name}.json`
4. The editor will now show the new profile name in the title
5. The modification indicator (*) will be cleared

**Note:** Use "Copy" to save your current work as a new profile with a different name.

## Project Structure

```
frontend/
├── public/              # Static assets
├── src/
│   ├── components/      # React components
│   │   ├── CurvesTab/   # Curve editor components
│   │   │   ├── CurveChart.jsx
│   │   │   ├── CurvesTab.jsx
│   │   │   └── CurvesTab.css
│   │   ├── Header.jsx
│   │   ├── Tabs.jsx
│   │   ├── EntitiesTab.jsx
│   │   ├── PointsTab.jsx
│   │   ├── OffsetSection.jsx
│   │   ├── PointRow.jsx
│   │   └── PreferencesModal.jsx
│   ├── utils/           # Utility functions
│   │   ├── curveUtils.js
│   │   ├── entityUtils.js
│   │   └── preferences.js
│   ├── App.jsx         # Main application component
│   ├── main.jsx        # Application entry point
│   └── index.css       # Global styles
├── index.html          # HTML template
├── vite.config.ts      # Vite configuration
├── Caddyfile           # Caddy web server configuration
├── package.json        # Dependencies and scripts
└── README.md           # This file
```

## Component Architecture

### App.jsx

Main application component that manages:

- Profile data state
- Active tab state
- File loading/saving
- Entity and point updates

### Header.jsx

Top toolbar with file operations:

- Load Profile: Load from file (.xlsx or .json)
- Save: Save changes to current profile (disabled when no modifications)
- Copy: Save as new profile with different name
- New Profile: Create new profile from template

### Tabs.jsx

Tab navigation component

### EntitiesTab.jsx

Entity configuration interface with validation

### PointsTab.jsx

Container for point editing grids, handles grouping by offset

### OffsetSection.jsx

Displays a group of points under an offset header with a data table

### PointRow.jsx

Individual editable row in the points grid

### CurvesTab/

Curve editing interface:

- **CurvesTab.jsx**: Main curve editor with selector, configuration, and point table
- **CurveChart.jsx**: SVG-based visual chart with draggable points

### PreferencesModal.jsx

User preferences dialog:

- Control station and outstation IP/port configuration
- Auto-expand sections toggle
- Settings persisted to localStorage

## Data Format

The application expects JSON files with the following structure:

```json
{
  "entities": {
    "meters": 1,
    "ders": 1,
    "inverters": 1,
    "batteries": 1
  },
  "binary_outputs": {
    "offsets": { "scada": 0, "gap_1": 586, ... },
    "points": [
      {
        "index": "BO0",
        "description": "...",
        "uid": "...",
        "purpose": "...",
        "value": 1,
        "associated_index": "BI11",
        "ieee_1815_2": true,
        "supported": true
      }
    ]
  },
  "binary_inputs": { ... },
  "analog_outputs": { ... },
  "analog_inputs": { ... }
}
```

## Development

### Available Scripts

- `npm run dev` - Start development server
- `npm run lint` - Run ESLint

### Adding New Features

1. Create new components in `src/components/`
2. Import and use them in `App.jsx` or other components

### State Management

The application uses React's built-in `useState` and `useEffect` hooks for state management. The main profile data is stored in the `App` component and passed down to child components via props.

## Browser Compatibility

This application works with modern browsers that support:

- ES6+ JavaScript
- CSS Grid and Flexbox
- File API
- React 18

Tested on:

- Chrome/Edge 90+
- Firefox 88+
- Safari 14+

## Performance Considerations

- Large profile files (thousands of points) render efficiently using React's virtual DOM
- Grid rendering is optimized with `useMemo` to avoid unnecessary re-renders
- File operations use the browser's native File API for optimal performance
