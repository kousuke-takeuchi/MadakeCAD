## What

<!-- One paragraph: what changes for the user. -->

## Why / spec

<!-- Link the spec section and plan you followed (docs/internal/specs/…, docs/superpowers/plans/…). If the behaviour changed, the spec is updated in this PR. -->

## Checklist

- [ ] Every edit to the drawing goes through a `Command` (no direct model mutation)
- [ ] Tests added/updated with bilingual spec lines; `python3 scripts/gen_spec.py` run and `docs/13-specification*.md` committed
- [ ] `cargo test` / `npx vitest run` / `npx vue-tsc --noEmit` (and `python3 -m unittest discover -s freecad-addon/tests` when the add-on changed) pass
- [ ] UI strings are in every catalog under `src/locales/` (no literals in components)
- [ ] Docs updated in English and Japanese where behaviour changed
