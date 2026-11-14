import cssContent from '@/assets/stylesheets/macFix.css?inline'
import { getBoot } from '@/platform/os'

export async function useCheckDisableMouseover() {
	if (getBoot().disable_mouseover) {
		const styleElement = document.createElement('style')
		styleElement.innerHTML = cssContent
		document.head.appendChild(styleElement)
	}
}
