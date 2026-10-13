import { useElementSize } from '@vueuse/core'
import { computed, onMounted, onUnmounted, type Ref } from 'vue'

export interface DynamicFontSizeOptions {
	containerElement: Ref<HTMLElement | null>
	text: Ref<string | undefined>
	baseFontSize?: number
	minFontSize?: number
	maxFontSize?: number
	availableWidthRatio?: number
	maxContainerWidth?: number
	padding?: number
	fontFamily?: string
	fontWeight?: string | number
}

export function useDynamicFontSize(options: DynamicFontSizeOptions) {
	const {
		containerElement,
		text,
		baseFontSize = 1.25,
		minFontSize = 0.75,
		maxFontSize = 2,
		availableWidthRatio = 0.9,
		maxContainerWidth = 400,
		padding = 24,
		fontFamily = 'inherit',
		fontWeight = 'inherit',
	} = options

	const { width: containerWidth } = useElementSize(containerElement)
	let measurementElement: HTMLElement | null = null

	const createMeasurementElement = () => {
		if (measurementElement) return measurementElement

		measurementElement = document.createElement('div')
		measurementElement.style.cssText = `
      position: absolute;
      top: -9999px;
      left: -9999px;
      opacity: 0;
      pointer-events: none;
      white-space: nowrap;
      font-family: ${fontFamily};
      font-weight: ${fontWeight};
    `
		measurementElement.setAttribute('aria-hidden', 'true')
		document.body.appendChild(measurementElement)

		return measurementElement
	}

	const cleanupMeasurementElement = () => {
		if (measurementElement?.parentNode) {
			measurementElement.parentNode.removeChild(measurementElement)
			measurementElement = null
		}
	}

	const measureTextWidth = (textContent: string, fontSize: number): number => {
		if (!textContent) return 0

		const element = createMeasurementElement()
		element.style.fontSize = `${fontSize}rem`
		element.textContent = textContent

		return element.getBoundingClientRect().width
	}

	// Width grows in step with font size, so one measurement per text is enough to size it for any
	// container width without forcing layout again.
	let measuredText: string | undefined
	let measuredWidthPerRem = 0

	const getWidthPerRem = (textContent: string): number => {
		if (textContent !== measuredText) {
			measuredWidthPerRem = measureTextWidth(textContent, baseFontSize) / baseFontSize
			measuredText = textContent
		}

		return measuredWidthPerRem
	}

	const fontSize = computed(() => {
		if (!text.value || !containerWidth.value) return `${baseFontSize}rem`

		const availableWidth =
			Math.min(containerWidth.value * availableWidthRatio, maxContainerWidth) - padding

		const widthPerRem = getWidthPerRem(text.value)
		if (widthPerRem * baseFontSize <= availableWidth) return `${baseFontSize}rem`

		const optimalSize = Math.min(Math.max(availableWidth / widthPerRem, minFontSize), maxFontSize)
		return `${optimalSize}rem`
	})

	onMounted(createMeasurementElement)
	onUnmounted(cleanupMeasurementElement)

	return {
		fontSize,
		containerWidth,
		cleanup: cleanupMeasurementElement,
	}
}
