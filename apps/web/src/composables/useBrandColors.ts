// TEMPORARY: Brand colors composable
// Allows real-time experimentation with brand colors
// This is a placeholder until a proper settings system is built
// DO NOT rely on this for production - it will be replaced

import { ref, computed, onMounted } from 'vue'

// Color definitions from variables.scss
interface ColorDefinition {
  name: string
  variable: string
  description: string
  defaultValue: string
}

interface ColorGroup {
  title: string
  colors: ColorDefinition[]
}

const colorGroups: ColorGroup[] = [
  {
    title: 'Brand Colors',
    colors: [
      { name: 'Brand', variable: 'color-brand', description: 'Main brand color', defaultValue: '#FFD700' },
      { name: 'Brand Highlight', variable: 'color-brand-highlight', description: '25% opacity highlight', defaultValue: 'rgba(255, 215, 0, 0.25)' },
      { name: 'Brand Shadow', variable: 'color-brand-shadow', description: 'Shadow/glow color', defaultValue: 'rgba(255, 215, 0, 0.7)' },
    ],
  },
  {
    title: 'Brand Gradients',
    colors: [
      { name: 'Gradient BG', variable: 'brand-gradient-bg', description: 'Sidebar background gradient', defaultValue: 'linear-gradient(135deg, #0a192f 0%, #1e3a5f 50%, #FFD700 100%)' },
      { name: 'Gradient Strong', variable: 'brand-gradient-strong-bg', description: 'Strong gradient variant', defaultValue: 'linear-gradient(270deg, #0a192f 0%, #1e3a5f 50%, #FFD700 100%)' },
      { name: 'Gradient Button', variable: 'brand-gradient-button', description: 'Button overlay', defaultValue: 'rgba(255, 215, 0, 0.3)' },
      { name: 'Gradient Border', variable: 'brand-gradient-border', description: 'Border accent', defaultValue: 'rgba(255, 215, 0, 0.3)' },
    ],
  },
  {
    title: 'Semantic Colors',
    colors: [
      { name: 'Red', variable: 'color-red', description: 'Errors, dangers', defaultValue: '#ed1148' },
      { name: 'Orange', variable: 'color-orange', description: 'Warnings', defaultValue: '#fe7e11' },
      { name: 'Green', variable: 'color-green', description: 'Success', defaultValue: '#1bd96a' },
      { name: 'Blue', variable: 'color-blue', description: 'Info, links', defaultValue: '#4f9cff' },
      { name: 'Purple', variable: 'color-purple', description: 'Special/premium', defaultValue: '#c78aff' },
    ],
  },
  {
    title: 'Surface Colors',
    colors: [
      { name: 'Surface 1', variable: 'surface-1', description: 'Base background', defaultValue: '#0a192f' },
      { name: 'Surface 2', variable: 'surface-2', description: 'Secondary backgrounds', defaultValue: '#112645' },
      { name: 'Surface 3', variable: 'surface-3', description: 'Raised surfaces', defaultValue: '#1a355b' },
      { name: 'Surface 4', variable: 'surface-4', description: 'Elevated surfaces', defaultValue: '#203e66' },
      { name: 'Surface 5', variable: 'surface-5', description: 'Borders, dividers', defaultValue: '#254771' },
    ],
  },
  {
    title: 'Text Colors',
    colors: [
      { name: 'Text Primary', variable: 'color-text-primary', description: 'Main text', defaultValue: '#ffffff' },
      { name: 'Text Default', variable: 'color-text-default', description: 'Default text', defaultValue: '#b0bac5' },
      { name: 'Text Tertiary', variable: 'color-text-tertiary', description: 'Muted text', defaultValue: '#96a2b0' },
    ],
  },
  {
    title: 'UI Component Colors',
    colors: [
      { name: 'Button BG', variable: 'color-button-bg', description: 'Button backgrounds', defaultValue: '#34363c' },
      { name: 'Link', variable: 'color-link', description: 'Link color', defaultValue: '#4f9cff' },
      { name: 'Tooltip BG', variable: 'color-tooltip-bg', description: 'Tooltip backgrounds', defaultValue: '#000000' },
      { name: 'Tooltip Text', variable: 'color-tooltip-text', description: 'Tooltip text', defaultValue: '#ffffff' },
    ],
  },
]

// Preset interface
interface Preset {
  id: string
  name: string
  colors: Record<string, string>
  createdAt: number
}

// Store presets in localStorage
const PRESETS_KEY = 'lodestone-brand-presets'

export function useBrandColors() {
  const colors = ref<Record<string, string>>({})
  const presets = ref<Preset[]>([])
  const currentPreset = ref<string | null>(null)

  // Load initial colors from CSS variables
  const loadColors = () => {
    const root = document.documentElement
    const allColors: Record<string, string> = {}

    colorGroups.forEach((group) => {
      group.colors.forEach((color) => {
        const value = root.style.getPropertyValue(`--${color.variable}`) || color.defaultValue
        allColors[color.variable] = value.trim()
      })
    })

    colors.value = allColors
  }

  // Load presets from localStorage
  const loadPresets = () => {
    try {
      const stored = localStorage.getItem(PRESETS_KEY)
      if (stored) {
        presets.value = JSON.parse(stored)
      }
    } catch (e) {
      console.error('Failed to load presets:', e)
    }
  }

  // Update a single color
  const updateColor = (variable: string, value: string) => {
    colors.value[variable] = value
    document.documentElement.style.setProperty(`--${variable}`, value)
  }

  // Update multiple colors at once
  const updateColors = (newColors: Record<string, string>) => {
    colors.value = { ...colors.value, ...newColors }
    Object.entries(newColors).forEach(([variable, value]) => {
      document.documentElement.style.setProperty(`--${variable}`, value)
    })
  }

  // Reset a single color to default
  const resetColor = (variable: string) => {
    const colorDef = colorGroups
      .flatMap((g) => g.colors)
      .find((c) => c.variable === variable)
    if (colorDef) {
      updateColor(variable, colorDef.defaultValue)
    }
  }

  // Reset all colors to defaults
  const resetAllColors = () => {
    colorGroups.forEach((group) => {
      group.colors.forEach((color) => {
        document.documentElement.style.setProperty(`--${color.variable}`, color.defaultValue)
      })
    })
    loadColors()
  }

  // Save current colors as a preset
  const savePreset = (name: string): string => {
    const preset: Preset = {
      id: Date.now().toString(),
      name,
      colors: { ...colors.value },
      createdAt: Date.now(),
    }
    presets.value = [...presets.value, preset]
    localStorage.setItem(PRESETS_KEY, JSON.stringify(presets.value))
    currentPreset.value = preset.id
    return preset.id
  }

  // Load a preset
  const loadPreset = (presetId: string) => {
    const preset = presets.value.find((p) => p.id === presetId)
    if (preset) {
      updateColors(preset.colors)
      currentPreset.value = presetId
    }
  }

  // Delete a preset
  const deletePreset = (presetId: string) => {
    presets.value = presets.value.filter((p) => p.id !== presetId)
    localStorage.setItem(PRESETS_KEY, JSON.stringify(presets.value))
    if (currentPreset.value === presetId) {
      currentPreset.value = null
    }
  }

  // Export to SCSS
  const exportSCSS = (): string => {
    let scss = `// Lodestone Brand Colors Export\n`
    scss += `// Generated: ${new Date().toISOString()}\n\n`
    scss += `:root {\n`

    colorGroups.forEach((group) => {
      scss += `  /* ${group.title} */\n`
      group.colors.forEach((color) => {
        const value = colors.value[color.variable] || color.defaultValue
        scss += `  --${color.variable}: ${value};\n`
      })
      scss += `\n`
    })

    scss += `}\n`
    return scss
  }

  // Export to JSON
  const exportJSON = (): string => {
    return JSON.stringify(
      {
        name: currentPreset.value ? presets.value.find((p) => p.id === currentPreset.value)?.name || 'Custom' : 'Custom',
        colors: colors.value,
        exportedAt: new Date().toISOString(),
      },
      null,
      2,
    )
  }

  // Download file helper
  const downloadFile = (content: string, filename: string, type: string) => {
    const blob = new Blob([content], { type })
    const url = URL.createObjectURL(blob)
    const a = document.createElement('a')
    a.href = url
    a.download = filename
    document.body.appendChild(a)
    a.click()
    document.body.removeChild(a)
    URL.revokeObjectURL(url)
  }

  // Export SCSS file
  const exportSCSSFile = () => {
    const scss = exportSCSS()
    downloadFile(scss, 'brand-colors.scss', 'text/scss')
  }

  // Export JSON file
  const exportJSONFile = () => {
    const json = exportJSON()
    downloadFile(json, 'brand-colors.json', 'application/json')
  }

  // Import preset from JSON
  const importPreset = (file: File) => {
    const reader = new FileReader()
    reader.onload = (e) => {
      try {
        const data = JSON.parse(e.target?.result as string)
        if (data.colors) {
          updateColors(data.colors)
          // Auto-save as preset
          const name = data.name || 'Imported Preset'
          savePreset(name)
        }
      } catch (err) {
        console.error('Failed to import preset:', err)
      }
    }
    reader.readAsText(file)
  }

  onMounted(() => {
    loadColors()
    loadPresets()
  })

  return {
    colors,
    presets,
    currentPreset,
    updateColor,
    updateColors,
    resetColor,
    resetAllColors,
    savePreset,
    loadPreset,
    deletePreset,
    exportSCSS,
    exportJSON,
    exportSCSSFile,
    exportJSONFile,
    importPreset,
    colorGroups,
  }
}
