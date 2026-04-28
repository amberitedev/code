<script setup lang="ts">
import { onMounted, onUnmounted, reactive, ref } from 'vue'
import ElementHighlight from './ElementHighlight.vue'

interface ComponentInfo {
	name: string
	file: string
	parent: { name: string; file: string } | null
}

const enabled = ref(false)
const highlight = reactive({ show: false, top: 0, left: 0, width: 0, height: 0, name: '' })

function getComponentInfo(el: Element): ComponentInfo | null {
	let node: any = el
	while (node) {
		const vc = node.__vueParentComponent
		if (vc) {
			return {
				name: vc.type?.name || vc.type?.__name || 'Anonymous',
				file: vc.type?.__file || '',
				parent: vc.parent
					? {
							name: vc.parent.type?.name || vc.parent.type?.__name || '',
							file: vc.parent.type?.__file || '',
						}
					: null,
			}
		}
		node = node.parentElement
	}
	return null
}

function onMouseMove(e: MouseEvent) {
	if (!enabled.value) return
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

function onClick(e: MouseEvent) {
	if (!enabled.value) return
	e.preventDefault()
	e.stopPropagation()
	const target = e.target as Element
	const info = getComponentInfo(target)
	const rect = target.getBoundingClientRect()
	window.dispatchEvent(
		new CustomEvent('design-inspector:pick', {
			detail: {
				component: info?.name || 'Unknown',
				file: info?.file || '',
				parent: info?.parent ?? null,
				rect: { top: rect.top, left: rect.left, width: rect.width, height: rect.height },
				html: target.outerHTML.slice(0, 500),
				cssClasses: Array.from(target.classList),
			},
		}),
	)
	enabled.value = false
	highlight.show = false
	document.body.style.cursor = ''
}

function onKeyDown(e: KeyboardEvent) {
	if (e.key === 'Alt' && !enabled.value) {
		e.preventDefault()
		enabled.value = true
		document.body.style.cursor = 'crosshair'
	}
	if (e.key === 'Escape' && enabled.value) {
		enabled.value = false
		highlight.show = false
		document.body.style.cursor = ''
	}
}

function onKeyUp(e: KeyboardEvent) {
	if (e.key === 'Alt' && enabled.value) {
		enabled.value = false
		highlight.show = false
		document.body.style.cursor = ''
	}
}

onMounted(() => {
	window.addEventListener('keydown', onKeyDown)
	window.addEventListener('keyup', onKeyUp)
	window.addEventListener('mousemove', onMouseMove)
	window.addEventListener('click', onClick, true)
})

onUnmounted(() => {
	window.removeEventListener('keydown', onKeyDown)
	window.removeEventListener('keyup', onKeyUp)
	window.removeEventListener('mousemove', onMouseMove)
	window.removeEventListener('click', onClick, true)
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
