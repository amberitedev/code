# packages/ui

Load `CLAUDE.md` for architecture and token details. Use the rules below as the fast path for UI work.

## Purpose

`packages/ui` is the shared UI system for both the website and the desktop app. It is the default place to look before building new UI.

## Required Workflow

- Search `src/components/` and `src/layouts/` before creating anything new.
- Reuse exported `@modrinth/ui` components whenever possible.
- If a screen is app-specific but visually standard, compose existing shared components instead of recreating them in `apps/app-frontend`.
- If a component could plausibly be used by both frontends, put it here instead of duplicating it in an app.

## Visual Guardrails

- Do not introduce a new font, spacing scale, radius system, shadow language, or color palette.
- Use existing surface and text tokens from the shared theme. Prefer token classes and CSS variables over hardcoded colors.
- For backgrounds, use the shared surface hierarchy from `CLAUDE.md`.
- Match the density and rhythm of nearby Modrinth UI. Avoid oversized padding, isolated one-off layouts, and bespoke card/button styling.
- Do not build custom replacements for buttons, inputs, cards, tabs, modals, dropdowns, tables, or settings rows when shared components already exist.

## Desktop App Fit

- Desktop app UI should feel native to the existing Modrinth App, not like a new product.
- Prefer established app patterns such as shared page layouts, `ButtonStyled`, shared modals, shared settings controls, and existing navigation pieces.
- When a page already has neighboring route patterns in `apps/app-frontend`, match those patterns first and only diverge for a concrete product reason.

## Escalation Rule

- If you think new UI is necessary, first confirm that no existing shared component or layout can be composed to solve it.
