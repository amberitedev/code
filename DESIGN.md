---
name: Amberite
description: A Minecraft mod launcher and server management application with client-to-client sync and core-to-client sync for mod management
version: 1.0.0
date: 2026-04-24

# Design tokens

## Colors

### Surfaces (Light Theme)
- surface-1: '#ebebeb'
- surface-1.5: '#ededed'
- surface-2: '#f5f5f5'
- surface-2.5: '#eef1f5'
- surface-3: '#f8f8f8'
- surface-4: '#ffffff'
- surface-5: '#dddddd'

### Surfaces (Dark Theme)
- surface-1: '#16181c'
- surface-1.5: '#1a1c20'
- surface-2: '#1d1f23'
- surface-2.5: '#222429'
- surface-3: '#27292e'
- surface-4: '#34363c'
- surface-5: '#42444a'

### Text Colors (Light Theme)
- text-primary: '#1a202c'
- text-default: '#2c2e31'
- text-tertiary: '#484d54'

### Text Colors (Dark Theme)
- text-primary: '#ffffff'
- text-default: '#b0bac5'
- text-tertiary: '#96a2b0'

### Brand Colors (Green)
- green-50: '#eefff6'
- green-100: '#d7ffeb'
- green-200: '#b2ffd9'
- green-300: '#76ffbc'
- green-400: '#33f598'
- green-500: '#09de78'
- green-600: '#00af5c'
- green-700: '#04914f'
- green-800: '#0a7141'
- green-900: '#0a5d38'
- green-950: '#00341d'

### Semantic Colors
- red: '#ed4661'
- orange: '#e08325'
- blue: '#2b79cc'
- purple: '#9f4dff'
- gray: '#686a72'

### Highlighter Colors (25% opacity)
- brand-highlight: 'rgba(0, 175, 92, 0.25)'
- red-highlight: 'rgba(203, 34, 69, 0.25)'
- orange-highlight: 'rgba(224, 131, 37, 0.25)'
- green-highlight: 'rgba(0, 175, 92, 0.25)'
- blue-highlight: 'rgba(31, 104, 192, 0.25)'
- purple-highlight: 'rgba(142, 50, 243, 0.25)'
- gray-highlight: 'rgba(89, 91, 97, 0.25)'

### UI Component Colors
- button-bg: 'var(--surface-4)'
- button-border: 'rgba(161, 161, 161, 0.35)'
- divider: 'var(--surface-5)'
- scrollbar: '#96a2b0'
- tooltip-bg: '#000000'
- tooltip-text: '#ecf9fb'

## Typography

### Font Stack
- font-family: "'Inter', 'Segoe UI', system-ui, -apple-system, sans-serif"

### Font Sizes
- text-xs: '0.75rem'
- text-sm: '0.875rem'
- text-base: '1rem'
- text-lg: '1.125rem'
- text-xl: '1.25rem'
- text-2xl: '1.5rem'
- text-3xl: '1.875rem'
- text-4xl: '2.25rem'

### Font Weights
- font-normal: '400'
- font-medium: '500'
- font-semibold: '600'
- font-bold: '700'
- font-extrabold: '800'

### Line Heights
- leading-none: '1'
- leading-tight: '1.25'
- leading-normal: '1.5'
- leading-relaxed: '1.625'

### Letter Spacing
- tracking-tighter: '-0.05em'
- tracking-tight: '-0.025em'
- tracking-normal: '0'
- tracking-wide: '0.025em'
- tracking-wider: '0.05em'

## Spacing

### Base Units
- spacing-0: '0'
- spacing-px: '1px'
- spacing-0.5: '0.125rem'
- spacing-1: '0.25rem'
- spacing-1.5: '0.375rem'
- spacing-2: '0.5rem'
- spacing-2.5: '0.625rem'
- spacing-3: '0.75rem'
- spacing-3.5: '0.875rem'
- spacing-4: '1rem'
- spacing-5: '1.25rem'
- spacing-6: '1.5rem'
- spacing-8: '2rem'
- spacing-10: '2.5rem'
- spacing-12: '3rem'
- spacing-16: '4rem'
- spacing-20: '5rem'
- spacing-24: '6rem'

### Layout-Specific Spacing
- left-bar-width: '4rem'
- top-bar-height: '3rem'
- right-bar-width: '300px'

## Border Radius

### Border Radii
- radius-none: '0'
- radius-xs: '0.25rem'
- radius-sm: '0.5rem'
- radius-md: '0.75rem'
- radius-lg: '1rem'
- radius-xl: '1.25rem'
- radius-2xl: '1.5rem'
- radius-full: '9999px'

### Semantic Radius
- radius-button: '0.75rem'
- radius-button-lg: '1rem'
- radius-button-sm: '0.5rem'
- radius-input: '0.5rem'
- radius-card: '1rem'
- radius-panel: '1.25rem'

## Elevation & Shadows

### Box Shadows
- shadow-none: 'none'
- shadow-xs: '0 1px 2px 0 rgba(0, 0, 0, 0.05)'
- shadow-sm: '0 1px 3px 0 rgba(0, 0, 0, 0.1), 0 1px 2px 0 rgba(0, 0, 0, 0.06)'
- shadow-md: '0 4px 6px -1px rgba(0, 0, 0, 0.1), 0 2px 4px -1px rgba(0, 0, 0, 0.06)'
- shadow-lg: '0 10px 15px -3px rgba(0, 0, 0, 0.1), 0 4px 6px -2px rgba(0, 0, 0, 0.05)'
- shadow-xl: '0 20px 25px -5px rgba(0, 0, 0, 0.1), 0 10px 10px -5px rgba(0, 0, 0, 0.04)'
- shadow-2xl: '0 25px 50px -12px rgba(0, 0, 0, 0.25)'
- shadow-inner: 'inset 0 2px 4px 0 rgba(0, 0, 0, 0.06)'
- shadow-button: '0 1px 3px 0 rgba(0, 0, 0, 0.05), 0 1px 2px 0 rgba(0, 0, 0, 0.15)'
- shadow-card: 'rgba(50, 50, 100, 0.1) 0px 2px 4px 0px'
- shadow-floating: 'hsla(0, 0%, 0%, 0) 0px 0px 0px 0px, hsla(0, 0%, 0%, 0) 0px 0px 0px 0px, hsla(0, 0%, 0%, 0.1) 0px 4px 6px -1px, hsla(0, 0%, 0%, 0.1) 0px 2px 4px -1px'

### Inset Shadows
- shadow-inset-sm: 'inset 0px -1px 2px hsla(221, 39%, 91%, 0.15)'
- shadow-inset: 'inset 0px -2px 2px hsla(221, 39%, 91%, 0.05)'
- shadow-inset-lg: 'inset 0px -2px 2px hsla(221, 39%, 91%, 0.1)'

## Motion & Timing

### Transitions
- transition-none: 'none'
- transition-all: 'all 0.2s'
- transition-colors: 'background-color 0.25s ease-in-out, color 0.25s ease-in-out, border-color 0.25s ease-in-out, fill 0.25s ease-in-out'
- transition-opacity: 'opacity 0.25s ease-in-out'
- transition-transform: 'transform 0.25s ease-in-out'
- transition-shadow: 'box-shadow 0.25s ease-in-out'
- transition-scale: 'scale 0.125s ease-in-out'

### Durations
- duration-75: '75ms'
- duration-100: '100ms'
- duration-150: '150ms'
- duration-200: '200ms'
- duration-300: '300ms'
- duration-500: '500ms'
- duration-700: '700ms'
- duration-1000: '1000ms'

### Easing Functions
- ease-linear: 'linear'
- ease-in: 'cubic-bezier(0.4, 0, 1, 1)'
- ease-out: 'cubic-bezier(0, 0, 0.2, 1)'
- ease-in-out: 'cubic-bezier(0.4, 0, 0.2, 1)'
- ease-in-out-slow: 'cubic-bezier(0.15, 1.4, 0.64, 0.96)'
- ease-bounce: 'cubic-bezier(0.51, 1.08, 0.35, 1.15)'
- ease-pop-out: 'cubic-bezier(0.68, -0.17, 0.23, 0.11)'

## Z-Index Scale
- z-0: '0'
- z-10: '10'
- z-20: '20'
- z-30: '30'
- z-40: '40'
- z-50: '50'
- z-dropdown: '1000'
- z-sticky: '1020'
- z-fixed: '1030'
- z-modal-backdrop: '1040'
- z-modal: '1050'
- z-popover: '1060'
- z-tooltip: '1070'
- z-toast: '1080'

---

# Design Intent & Look & Feel

## Overall Aesthetic

Amberite embraces a modern, clean interface philosophy inspired by Material Design principles while maintaining its own distinctive identity. The design language balances technical sophistication with approachability, making mod management feel accessible to both novice and experienced Minecraft players.

## Visual Hierarchy & Depth

The interface employs a layered surface system that creates clear depth perception:

- Deep backgrounds (surface-1, surface-2) establish the base canvas
- Mid-level surfaces (surface-3) handle interactive elements like headers and floating bars
- Elevated surfaces (surface-4, surface-5) contain actionable content like cards and buttons

This layering ensures users can immediately identify interactive elements while maintaining visual coherence.

## Brand Identity

The green brand color (#00af5c in light mode, #1bd96a in dark mode) represents growth, stability, and the Minecraft ecosystem. Used sparingly for:

- Primary call-to-action buttons
- Active navigation states
- Success indicators
- Brand accents and logos

The color is intentionally muted in light mode to reduce eye strain during extended play sessions, while dark mode allows for more vibrant saturation.

## Typography Voice

Text hierarchy follows a practical approach:

- **Primary headings** use high contrast (white in dark mode, near-black in light mode)
- **Body text** maintains comfortable readability with slightly reduced contrast
- **Secondary information** uses tertiary text colors for visual de-emphasis

Font weights lean toward semibold (600) for interactive elements, ensuring clickable items feel substantial without appearing aggressive.

## Interaction Patterns

Buttons employ a consistent interaction model:

- Standard buttons fill backgrounds with brand color on primary actions
- Outlined buttons maintain transparency until hovered
- Subtle scale transformations (95%) provide tactile feedback on click
- Hover brightness adjustments ensure state changes are perceptible but not jarring

## Navigation Experience

The left sidebar navigation uses icon-only buttons (64px wide) to maximize content area. Top navigation bar (48px tall) contains:

- Back/forward navigation
- Breadcrumb trail
- Window controls
- Quick actions

This three-panel layout (sidebar, content area, optional right sidebar) balances quick access with focus on content.

## Motion Philosophy

Animations prioritize utility over showmanship:

- Page transitions use subtle 250ms fades
- Navigation elements employ elastic "pop" animations (500ms) to draw attention without distraction
- Loading states use progress bars rather than spinners
- Modal transitions use smooth scale transforms for focus

Animations respect `prefers-reduced-motion` preferences, disabling them for accessibility.

## Component Consistency

All interactive components share common traits:

- Consistent border radius (8px for buttons, 16px for cards)
- Uniform padding scales (4px, 8px, 12px, 16px)
- Predictable color state changes
- Clear disabled states at 50% opacity

This ensures users build muscle memory across the application.

## Theme Support

Full light and dark theme support with OLED-compatible dark mode:

- Light mode uses warm grays with slight warmth
- Dark mode provides true dark backgrounds for OLED panels
- Brand colors automatically adjust saturation between themes
- Surface hierarchy reverses appropriately

## Platform Integration

Desktop-specific considerations:

- Native window controls on supported platforms
- macOS-style traffic light buttons when detected
- Proper scrollbar styling through overlay-scrollbars
- Respect for system-level appearance preferences

---

# Implementation Notes

## CSS Custom Properties

All design tokens are exposed as CSS custom properties for runtime theme switching. Key tokens follow the pattern `--{category}-{name}`:

```css
:root {
  /* Surfaces */
  --surface-1: #ebebeb;
  --surface-4: #ffffff;

  /* Brand */
  --color-brand: #00af5c;
  --color-brand-highlight: rgba(0, 175, 92, 0.25);

  /* Layout */
  --top-bar-height: 3rem;
  --left-bar-width: 4rem;
}
```

## Dark Mode Activation

Dark mode activates via:

- `.dark-mode` class on html element
- `.dark` class on html element
- `data-theme="dark"` attribute on html element
- `.oled-mode` class for OLED-optimized dark

## Tailwind Integration

Tokens integrate with Tailwind CSS through `@tailwindcss/theme()`:

- Colors map to `theme('colors.*')`
- Spacing maps to `theme('spacing.*')`
- Border radius maps to `theme('borderRadius.*')`
- Box shadow maps to `theme('boxShadow.*')`

## File Structure

The design system is defined across multiple source files:

- `packages/assets/styles/variables.scss` - CSS custom properties for all themes
- `packages/tooling-config/tailwind/tailwind-preset.ts` - Tailwind configuration
- `packages/ui/src/components/base/*.vue` - Reusable UI components
- `packages/ui/src/styles/tailwind*.css` - Global Tailwind utilities