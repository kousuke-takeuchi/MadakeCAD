# Standards & Output

[日本語](04-standards-output.ja.md)

Drawings comply with Japanese industrial conventions (JIS) and are rendered identically on screen and in print-quality vector output.

## Available

- **JIS drawing frame**: paper sizes A4–A0, landscape/portrait; border with zone references (columns numbered, rows lettered); title block (drawing number, title, scale, date, designed/drawn/checked/approved, company, revision mark)
- **Revision table**: ISO 7200-style block directly above the title block — columns mark / date / description / approved, oldest row at the bottom, up to six rows on the frame (older rows stay in the file); the title-block revision mark always follows the newest row. Edited in the revision dialog (Schematic tab → Edit group → *Revisions*), so saving is a single undoable command
- **Sheets & zones**: configurable zone grid per sheet; zone addresses are what cross-references point at
- **Cross-sheet references**: net labels of the same name are one project-wide net; each label carries the address of its counterpart on the other sheets (`/2.B3`, several listed side by side) on screen, in SVG and in PDF. Selecting a label lists the destinations in the properties panel, and clicking one jumps to that sheet and zooms to the matching label. Project-wide nets also drive the wire list and ERC, so a net spanning two sheets is reported once
- **SVG export**: millimeter-true (1:1 viewBox), print-quality vector output; the SVG renderer is the single source of truth for printed appearance
- **PDF export**: vector conversion of the SVG with embedded fonts (CJK-capable), one page per sheet
- **Bill of materials (CSV)**: reference designators aggregated with part numbers
- **Wire list (CSV)**: sheet, wire number, harness, wire part number, color, gauge (sq), length

## Planned

- **Terminal charts**: per-terminal-strip connection tables as reports and as generated drawing sheets (M4)
- **Extended report set**: from–to wire lists, cable summaries, cross-reference reports, BOM enriched from the parts database; reports emitted as framed drawing sheets and combined PDF (M4)
- **Customizable frame/title-block templates** so organizations can match their own formats (M4)
- IEC / ISO variants: IEC 60617 symbol styles, IEC 81346 structure designations, ISO 7200 title blocks and ISO 5457 sheet frames (long term)
