import { ref, type Ref } from 'vue'
import { requestComponentPick } from './useComponentPicker'
import { insertToken, extractRefs, type PickedRef } from './useComponentAutocomplete'
import { assignColor, makeColorKey } from './usePickColors'
import { getKeyStyles } from './useElementInfo'

export { extractRefs, type PickedRef }

export function useInlinePickMode(inputRef: Ref<HTMLElement | null>, bubbleId: string) {
	const picking = ref(false)

	function start() {
		picking.value = true
		requestComponentPick((info) => {
			picking.value = false
			const el = inputRef.value
			if (!el) return
			const name = info.name ?? 'element'

			// Build semantic key including live element identity — two distinct DOM nodes
			// always get distinct keys (and thus distinct colours), even with identical metadata.
			const colorKey = makeColorKey({
				source_file: info.source_file,
				parentFile: info.parentFile,
				parentLine: info.parentLine,
				props: info.props,
				tag: info.tag,
				element: info.element,
			})
			const color = assignColor(colorKey)

			// Dispatch sticky-pick so DesignInspectorRoot creates the persistent overlay.
			if (info.element) {
				window.dispatchEvent(new CustomEvent('design-inspector:sticky-pick', {
					detail: {
						element: info.element,
						name,
						sourceFile: info.source_file,
						sourceLines: info.source_lines,
						parentFile: info.parentFile ?? null,
						parentLine: info.parentLine ?? null,
						props: info.props ?? null,
						html: info.html ?? '',
						tag: info.tag ?? info.element.tagName.toLowerCase(),
						color,
						colorKey,
						bubbleId,
					},
				}))
			}

			// Move cursor to end then insert the rich token with computed style metadata.
			el.focus()
			const r = document.createRange()
			r.selectNodeContents(el)
			r.collapse(false)
			window.getSelection()?.removeAllRanges()
			window.getSelection()?.addRange(r)
			insertToken(el, null, name, '#', {
				sourceFile: info.source_file ?? undefined,
				sourceLines: info.source_lines,
				parentFile: info.parentFile ?? undefined,
				parentLine: info.parentLine ?? undefined,
				props: info.props,
				html: info.html,
				tag: info.tag,
				color,
				colorKey,
				computedStyles: info.element ? getKeyStyles(info.element) : undefined,
			})
		})
		window.addEventListener('design-inspector:bubble-pick-cancelled', () => {
			picking.value = false
		}, { once: true })
	}

	return { picking, start }
}
