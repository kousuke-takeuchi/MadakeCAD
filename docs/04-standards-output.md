# Standards & Output

[日本語](04-standards-output.ja.md)

Drawings comply with Japanese industrial conventions (JIS) and are rendered identically on screen and in print-quality vector output.

## Available

- **JIS drawing frame**: paper sizes A4–A0, landscape/portrait; border with zone references (columns numbered, rows lettered); title block (drawing number, title, scale, date, designed/drawn/checked/approved, company, revision mark)
- **Sheets & zones**: configurable zone grid per sheet; zone addresses drive future cross-references
- **SVG export**: millimeter-true (1:1 viewBox), print-quality vector output; the SVG renderer is the single source of truth for printed appearance
- **PDF export**: vector conversion of the SVG with embedded fonts (CJK-capable), one page per sheet
- **Bill of materials (CSV)**: reference designators aggregated with part numbers
- **Wire list (CSV)**: wire part number, color, gauge (sq), length

## Planned

- **Revision table**: rendering of the revision history block on the frame and an editing UI (data model already exists) (M2)
- **Cross-sheet references**: net labels annotated with the destination sheet/zone address, project-wide net continuity (M2)
- **Terminal charts**: per-terminal-strip connection tables as reports and as generated drawing sheets (M4)
- **Extended report set**: from–to wire lists, cable summaries, cross-reference reports, BOM enriched from the parts database; reports emitted as framed drawing sheets and combined PDF (M4)
- **Customizable frame/title-block templates** so organizations can match their own formats (M4)
- IEC-variant symbol/frame styles (long term)
