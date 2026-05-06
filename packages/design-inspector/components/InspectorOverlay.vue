<script setup lang="ts">
import { onMounted, onUnmounted, reactive, ref } from 'vue'
import ElementHighlight from './ElementHighlight.vue'
import { getComponentInfo, parseInspectorInfo, getKeyStyles } from '../composables/useElementInfo'
import { cancelComponentPick } from '../composables/useComponentPicker'

const diLog = import.meta.env.DEV
	? (...args: unknown[]) => console.log('[DesignInspector][InspectorOverlay]', ...args)
	: () => {}

const enabled = ref(false)
const pickForBubble = ref(false)
const highlight = reactive({ show: false, top: 0, left: 0, width: 0, height: 0, name: '' })

function onMouseMove(e: MouseEvent) {
	if (!enabled.value && !pickForBubble.value) return
	const target = document.elementFromPoint(e.clientX, e.clientY)
	if (!target || target === document.body) return
	const rect = target.getBoundingClientRect()
	const info = getComponentInfo(target)
	highlight.show = true
	highlight.top = rect.top
	highlight.left = rect.left
	highlight.width = rect.width
	highlight.height = rect.height
	highlight.name = info?.name || ''
}

function onMouseDown(e: Event) {
	if (!enabled.value && !pickForBubble.value) return
	e.preventDefault()   // prevents :active CSS state and default browser press behaviour
	e.stopPropagation()
}

function onClick(e: MouseEvent) {
	if (!enabled.value && !pickForBubble.value) return
	e.preventDefault()
	e.stopPropagation()
	const target = e.target as Element
	const info = getComponentInfo(target)
	const inspector = parseInspectorInfo(target)

	if (pickForBubble.value) {
		pickForBubble.value = false
		highlight.show = false
		document.body.style.cursor = ''
		// Derive best available name: explicit name > name-from-file > tag name
		const rawFile = info?.file || inspector?.file || null
		const nameFromFile = rawFile ? rawFile.split('/').pop()?.replace(/\.\w+$/, '') ?? null : null
		const resolvedName = (info?.name && info.name !== 'Anonymous') ? info.name : (nameFromFile ?? target.tagName.toLowerCase())
		diLog('Picked for bubble:', resolvedName, rawFile)
		// Collect primitive props for instance identification
		const props = info?.props && Object.keys(info.props).length > 0 ? info.props : undefined
		window.dispatchEvent(new CustomEvent('design-inspector:bubble-pick-result', {
			detail: {
				name: resolvedName,
				source_file: rawFile,
				source_lines: inspector?.lines,
				element: target,
				parentFile: inspector?.file ?? null,
				parentLine: inspector?.lines?.[0] ?? null,
				props,
				html: (target.cloneNode(false) as Element).outerHTML.slice(0, 200),
				tag: target.tagName.toLowerCase(),
			},
		}))
		return
	}

	diLog('Picked element:', info)
	window.dispatchEvent(new CustomEvent('design-inspector:pick', {
		detail: {
			component: info?.name || 'Unknown',
			file: info?.file || '',
			parent: info?.parent ?? null,
			element: target,
			html: (target.cloneNode(false) as Element).outerHTML,
			cssClasses: Array.from(target.classList),
			sourceLines: inspector?.lines,
			computedStyles: getKeyStyles(target),
			componentProps: info?.props ?? {},
		},
	}))
}

function onKeyDown(e: KeyboardEvent) {
	if (e.key === 'Escape') {
		if (pickForBubble.value) {
			pickForBubble.value = false
			highlight.show = false
			document.body.style.cursor = ''
			cancelComponentPick()
			window.dispatchEvent(new CustomEvent('design-inspector:bubble-pick-cancelled'))
			e.preventDefault()
			e.stopPropagation()
			return
		}
		if (enabled.value) {
			enabled.value = false
			highlight.show = false
			document.body.style.cursor = ''
			e.stopPropagation()
			e.preventDefault()
			diLog('Inspector DISABLED (Escape)')
			return
		}
		const closeEvent = new CustomEvent('design-inspector:close-newest', { cancelable: true })
		window.dispatchEvent(closeEvent)
		if (closeEvent.defaultPrevented) {
			e.stopPropagation()
			e.preventDefault()
		}
	}
	// Plain Alt only — Win+Alt (metaKey) is handled by CommentBubble for inline picking
	if (e.key === 'Alt' && !enabled.value && !e.metaKey) {
		e.preventDefault()
		enabled.value = true
		document.body.style.cursor = 'crosshair'
		diLog('Inspector ENABLED (Alt held)')
	}
}

function onKeyUp(e: KeyboardEvent) {
	if (e.key === 'Alt' && enabled.value && !pickForBubble.value) {
		enabled.value = false
		highlight.show = false
		document.body.style.cursor = ''
		diLog('Inspector DISABLED (Alt released)')
	}
}

function onStartPickForBubble() {
	pickForBubble.value = true
	document.body.style.cursor = 'crosshair'
	diLog('Pick-for-bubble mode ENABLED (Win+Alt)')
}

onMounted(() => {
	window.addEventListener('mousedown', onMouseDown, { capture: true })
	window.addEventListener('pointerdown', onMouseDown, { capture: true })
	window.addEventListener('keydown', onKeyDown, { capture: true })
	window.addEventListener('keyup', onKeyUp)
	window.addEventListener('mousemove', onMouseMove)
	window.addEventListener('click', onClick, true)
	window.addEventListener('design-inspector:start-pick-for-bubble', onStartPickForBubble)
	diLog('InspectorOverlay mounted')
})

onUnmounted(() => {
	window.removeEventListener('mousedown', onMouseDown, { capture: true })
	window.removeEventListener('pointerdown', onMouseDown, { capture: true })
	window.removeEventListener('keydown', onKeyDown, { capture: true })
	window.removeEventListener('keyup', onKeyUp)
	window.removeEventListener('mousemove', onMouseMove)
	window.removeEventListener('click', onClick, true)
	window.removeEventListener('design-inspector:start-pick-for-bubble', onStartPickForBubble)
	document.body.style.cursor = ''
})
</script>

<template>
	<ElementHighlight
		v-if="highlight.show"
		:top="highlight.top"
		:left="highlight.left"
		:width="highlight.width"
		:height="highlight.height"
		:name="highlight.name"
	/>
</template>
