import 'ace-builds/esm-resolver'

import cssText from '@modrinth/assets/styles/ace.css?raw'
import ace from 'ace-builds'

// @ts-expect-error ace.define exists at runtime but is missing from the ace-builds types
ace['define'](
	'ace/theme/modrinth',
	['require', 'exports', 'module', 'ace/lib/dom'],
	function (require: typeof ace.require, exports: Record<string, unknown>, _module: unknown) {
		exports.isDark = false
		exports.cssClass = 'ace-modrinth'
		exports.cssText = cssText

		const dom = require('ace/lib/dom')
		dom.importCssString(exports.cssText, exports.cssClass, false)
	},
)
