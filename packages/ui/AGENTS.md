# packages/ui

Shared Vue UI used by both `apps/frontend` and `apps/app-frontend`. The package contains reusable components, cross-platform page layouts, providers, composables, styles, locales, and Storybook stories.

## Structure

```text
src/components/   Reusable UI grouped by feature
src/layouts/      Shared page systems and wrapped route-level layouts
src/providers/    Dependency-injection contexts for platform behavior
src/composables/  Shared Vue behavior
src/styles/       Tailwind entrypoints and utilities
src/locales/      UI translations
src/stories/      Storybook stories
```

Public exports flow through each area's `index.ts` and the package-root `index.ts`. Code inside `src/layouts/` uses the `#ui/*` alias for other package modules.

## Common components

- Actions and forms: `ButtonStyled`, `StyledInput`, `Combobox`, `DropdownSelect`, `Checkbox`, `Toggle`, `RadioButtons`, `OptionGroup`, and file-input components.
- Navigation and overlays: `NavTabs`, `OverflowMenu`, `PopoutMenu`, `NewModal`, and the modal stack.
- Content and feedback: `Card`, `Table`, `EmptyState`, `Admonition`, `ReadyTransition`, ghost components, loading indicators, and notification providers.
- Page composition: `ContentPageHeader`, shared content/files/console/settings layouts, and wrapped hosting-management layouts.

`src/providers/` contains injected platform services such as the Modrinth client, notifications, and modal behavior. This lets the website and desktop app supply different implementations to the same shared layout.

The UI package imports `@amberite/amberite-api` for shared Amberite backend contracts used by cross-platform providers and layouts. Keep that workspace dependency declared in `packages/ui/package.json` and use the shared API instead of duplicating backend contracts in UI code.

## Styling

The package uses the shared Tailwind preset in `packages/tooling-config/tailwind/tailwind-preset.ts`. Theme surfaces, text colors, brand colors, radii, gaps, and shadows come from `packages/assets/styles/variables.scss`, including light, dark, and OLED themes.

The main visual primitives are layered `surface-*` backgrounds, `text-contrast`/`text-primary`/`text-secondary`, semantic status colors, and the orange `brand` color. `ButtonStyled`, form controls, menus, tables, cards, and modals already encode the standard interaction states and geometry.

## Rule

Never use generic browser-default controls or unstyled pure-HTML UI such as native dropdown selectors, checkboxes, radio buttons, file inputs, dialogs, alerts, or context menus. Use an existing styled component, or implement an accessible styled component when none fits.
