# Excel Template for Profile Creation

This document describes the Excel file format for creating new MESA profiles.

## File Structure

The Excel file should contain **5 sheets** with the following names:

1. **Key** - Entity counts (specific cell locations)
2. **BO** - Binary Outputs
3. **BI** - Binary Inputs
4. **AO** - Analog Outputs
5. **AI** - Analog Inputs

## Sheet Mappings

| Excel Sheet Name | JSON Section   | Notes                        |
| ---------------- | -------------- | ---------------------------- |
| Key              | entities       | Uses specific cell addresses |
| BO               | binary_outputs | Data parsed as table         |
| BI               | binary_inputs  | Data parsed as table         |
| AO               | analog_outputs | Data parsed as table         |
| AI               | analog_inputs  | Data parsed as table         |

### 1. Key Sheet (Entities)

The **Key** sheet contains entity counts at **specific cell locations**:

| Cell    | Content                      | Example Value |
| ------- | ---------------------------- | ------------- |
| R21     | Label: "No. of Meters (HM)"  |               |
| **R21** | **Meters count**             | **10**        |
| R24     | Label: "No. DER Units (HDU)" |               |
| **R24** | **DER count**                | **7**         |
| R27     | Label: "No. Inverters (HI)"  |               |
| **R27** | **Inverters count**          | **2**         |
| R30     | Label: "No. Batteries (HB)"  |               |
| **R30** | **Batteries count**          | **2**         |

**Important:** The parser reads values directly from cells **R21, R24, R27, and R30**.

**Fallback:** If the "Key" sheet is not found or cells are empty, the parser will attempt flexible parsing that looks for entity labels anywhere in the sheet.

### 2. BO Sheet (Binary Outputs)

**Offset Rows** (optional - if not provided, defaults will be used):

| offset_name          | offset_value |
| -------------------- | ------------ |
| scada                | 0            |
| gap_1                | 586          |
| schedules_v1         | 2000         |
| status_v1            | 2300         |
| schedules_v2         | 3000         |
| status_v2            | 3500         |
| gap_2                | 4000         |
| historical_meters    | 5000         |
| historical_ders      | 10000        |
| historical_inverters | 15000        |
| historical_batteries | 20000        |
| gap_3                | 30000        |
| vendor               | 50000        |

**Point Rows:**

| index | description                       | uid                                   | purpose | value | associated_index | ieee_1815_2 | ieee_1547_1 | supported |
| ----- | --------------------------------- | ------------------------------------- | ------- | ----- | ---------------- | ----------- | ----------- | --------- |
| BO0   | System Set Lockout State          | DSTO.DEROpSt.disconnected_and_blocked | State   | 1     | BI11             | TRUE        | FALSE       | TRUE      |
| BO1   | System Initiate Start-up Sequence | DCTE.CeaEgzReqCtl.return_to_service   | Cease   | 1     | BI12             | TRUE        | FALSE       | TRUE      |

### 3. BI Sheet (Binary Inputs)

Same structure as BO sheet, but with binary input points (BI0, BI1, etc.).

### 4. AO Sheet (Analog Outputs)

Same structure as Binary Outputs, but include a `units` column:

| index | description           | uid       | purpose | value | associated_index | ieee_1815_2 | ieee_1547_1 | supported | units |
| ----- | --------------------- | --------- | ------- | ----- | ---------------- | ----------- | ----------- | --------- | ----- |
| AO0   | Active Power Setpoint | DGEN.WSet | Control | 0.0   | AI0              | TRUE        | TRUE        | TRUE      | W     |

### 5. AI Sheet (Analog Inputs)

Same structure as AO sheet, with analog input points (AI0, AI1, etc.).

## Column Definitions

### Required Columns for Points

- **index** (string): Point identifier (e.g., "BO0", "BI0", "AO0", "AI0")
- **description** (string): Human-readable description of the point
- **uid** (string): Unique identifier following IEEE 1815.2 naming convention
- **purpose** (string): Purpose category (e.g., "State", "Control", "Monitoring")
- **value** (number): Default value for the point

### Optional Columns for Points

- **associated_index** (string): Related point index
- **ieee_1815_2** (boolean): Compliance with IEEE 1815.2 (TRUE/FALSE, 1/0, yes/no, X/blank)
- **ieee_1547_1** (boolean): Compliance with IEEE 1547.1
- **supported** (boolean): Whether the point is supported
- **units** (string): Units of measurement (analog points only)

## Boolean Values

Boolean columns accept multiple formats:

- `TRUE` / `FALSE`
- `1` / `0`
- `yes` / `no`
- `X` / blank
- Case insensitive

## Sheet Name Conventions

**Recommended sheet names (simple):**

- `Entities`, `BO`, `BI`, `AO`, `AI`

**Alternative names also supported:**

- Entities: First sheet, or any sheet containing "entities"
- BO: "binary_outputs" or any sheet with "binary" + "output"
- BI: "binary_inputs" or any sheet with "binary" + "input"
- AO: "analog_outputs" or any sheet with "analog" + "output"
- AI: "analog_inputs" or any sheet with "analog" + "input"

Sheet names are **case-insensitive**.

## Tips

1. **Headers are case-insensitive**: "Index", "index", and "INDEX" all work
2. **Column order doesn't matter**: Columns can be in any order
3. **Extra columns are ignored**: You can include additional columns for notes
4. **Missing optional fields**: Will use defaults (empty string or false)
5. **Offset rows**: Can be mixed with point rows in the same sheet

## Example Workflow

1. Create Excel file with 5 sheets
2. Fill in entity counts in first sheet
3. Add offset definitions (or skip for defaults)
4. Add point rows with required columns
5. Save as `.xlsx` or `.xls`
6. In MESA Profile Editor, click "New Profile"
7. Select your Excel file
8. Enter desired filename
9. Profile is created and downloaded

## Error Handling

If the Excel file cannot be parsed:

- Check that all required sheets exist
- Verify column names match expected format
- Ensure boolean values are in supported format
- Check that numeric values are valid numbers
- Validate that index values are unique
