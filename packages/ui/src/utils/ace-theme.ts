import 'ace-builds/esm-resolver'

import cssText from '@modrinth/assets/styles/ace.css?raw'
import type ace from 'ace-builds'

import { aceDefine } from './ace-define'

aceDefine(
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
