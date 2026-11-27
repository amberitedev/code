// The lint rules behind docs/internals/testing.md, ported from T3 Code's oxlint plugin.
// vite.config.ts turns them on for test files.

import { definePlugin, defineRule, type ESTree } from '@oxlint/plugins'

type Node = ESTree.Node

const TESTS = new Set(['describe', 'it', 'suite', 'test'])
const SKIPPED_TESTS = new Set(['xdescribe', 'xit', 'xtest'])
const FOCUSED_TESTS = new Set(['fdescribe', 'fit'])
const HOOKS = new Set(['after', 'afterAll', 'afterEach', 'before', 'beforeAll', 'beforeEach'])
const TIMERS = new Set(['setImmediate', 'setInterval', 'setTimeout'])
const TIMER_PROMISES = new Set(['node:timers/promises', 'timers/promises'])
const FUNCTIONS = new Set(['ArrowFunctionExpression', 'FunctionDeclaration', 'FunctionExpression'])
const QUARANTINE = /^\s*quarantine: #\d+ \S/

const WAIT_MESSAGE =
	'A test never waits on a timer, a sleep or a poll. Await the event that marks the milestone.'
const TIMEOUT_MESSAGE =
	'No per-test timeout or retry: the project timeout is the ceiling, and a flaky test is fixed, not retried.'
const SKIP_MESSAGE =
	'A skipped test needs a tracking issue on the line above: `// quarantine: #1234 <why>`.'
const ONLY_MESSAGE = 'A focused test stops the rest of the suite from running.'

function unwrap(node: Node | null | undefined): Node | undefined {
	let current = node ?? undefined
	while (
		current &&
		(current.type === 'ChainExpression' ||
			current.type === 'ParenthesizedExpression' ||
			current.type === 'TSAsExpression' ||
			current.type === 'TSNonNullExpression' ||
			current.type === 'TSSatisfiesExpression')
	) {
		current = current.expression
	}
	return current
}

/** A member or property key, when it is written out: `a.b`, `a['b']`, `{ b }`, never `a[b]`. */
function keyName(node: Node, computed = false): string | undefined {
	if (node.type === 'Identifier' && !computed) return node.name
	if (node.type === 'Literal' && typeof node.value === 'string') return node.value
	return undefined
}

/** `it.skip.each(cases)` as `['it', 'skip', 'each']`, or nothing when the root is not a name. */
function calleePath(node: Node | null | undefined): ReadonlyArray<string> | undefined {
	const expression = unwrap(node)
	if (!expression) return undefined
	if (expression.type === 'Identifier') return [expression.name]
	if (expression.type === 'CallExpression') return calleePath(expression.callee)
	if (expression.type !== 'MemberExpression') return undefined
	const property = keyName(expression.property, expression.computed)
	const object = calleePath(expression.object)
	return property !== undefined && object ? [...object, property] : undefined
}

/** The value of `key` when `node` is an object literal that sets it. */
function option(node: Node | undefined, keys: ReadonlySet<string>): Node | undefined {
	const expression = unwrap(node)
	if (expression?.type !== 'ObjectExpression') return undefined
	for (const property of expression.properties) {
		if (property.type !== 'Property') continue
		const key = keyName(property.key, property.computed)
		if (key !== undefined && keys.has(key)) return property.value
	}
	return undefined
}

/** `true` or a reason string turns a node:test option on; a variable is a platform gate. */
const isOn = (node: Node | undefined) => {
	const expression = unwrap(node)
	return (
		expression?.type === 'Literal' &&
		(expression.value === true || typeof expression.value === 'string')
	)
}

function statementOf(node: Node): Node {
	let current = node
	while (current.parent && !/(?:Statement|Declaration)$/.test(current.type))
		current = current.parent
	return current
}

/** Visits `node` and what it holds, except nested functions, which run on their own schedule. */
function some(node: unknown, match: (node: Node) => boolean): boolean {
	if (Array.isArray(node)) return node.some((child) => some(child, match))
	if (typeof node !== 'object' || node === null || !('type' in node)) return false
	const current = node as Node
	if (match(current)) return true
	if (FUNCTIONS.has(current.type)) return false
	return Object.entries(current).some(([key, value]) => key !== 'parent' && some(value, match))
}

const noTestTimeoutOrUnquarantinedSkip = defineRule({
	meta: {
		type: 'problem',
		docs: {
			description:
				'Disallow per-test timeouts and retries and focused tests, and require a quarantine issue for a skipped test.',
		},
	},
	create(context) {
		const skipped = (node: Node) => {
			const comments = context.sourceCode.getCommentsBefore(statementOf(node))
			if (comments.some((comment) => QUARANTINE.test(comment.value))) return
			context.report({ node, message: SKIP_MESSAGE })
		}
		return {
			CallExpression(node) {
				const path = calleePath(node.callee)
				if (!path) return
				const [root, ...members] = path
				const [, second, third] = node.arguments
				if (root === 'vi' && members[0] === 'setConfig') {
					const keys = new Set(['hookTimeout', 'retry', 'testTimeout'])
					if (option(node.arguments[0], keys)) context.report({ node, message: TIMEOUT_MESSAGE })
					return
				}
				// Whatever follows a hook's function is its timeout or options.
				if (HOOKS.has(root!) && members.length === 0) {
					if (node.arguments.length > 1) context.report({ node, message: TIMEOUT_MESSAGE })
					return
				}
				if (!TESTS.has(root!)) return
				// `test(name, options, fn)`, or `test(name, fn, options)`, where the options may be a
				// bare timeout. Only an object of flags that set no limit may follow the function.
				const last = node.arguments.findLastIndex((argument) => {
					const expression = unwrap(argument)
					return (
						expression?.type === 'ArrowFunctionExpression' ||
						expression?.type === 'FunctionExpression'
					)
				})
				const after = last < 0 ? [] : node.arguments.slice(last + 1)
				const flags = new Set(['concurrent', 'fails', 'only', 'sequential', 'skip', 'todo'])
				const limited =
					option(second, new Set(['retry', 'timeout'])) ||
					after.some((argument) => {
						const expression = unwrap(argument)
						return (
							expression?.type !== 'ObjectExpression' ||
							expression.properties.some(
								(property) =>
									property.type !== 'Property' ||
									!flags.has(keyName(property.key, property.computed) ?? ''),
							)
						)
					})
				if (limited) context.report({ node, message: TIMEOUT_MESSAGE })
				const options = [second, third]
				if (options.some((argument) => isOn(option(argument, new Set(['only']))))) {
					context.report({ node, message: ONLY_MESSAGE })
				} else if (options.some((argument) => isOn(option(argument, new Set(['skip', 'todo']))))) {
					skipped(node)
				}
			},
			Identifier(node) {
				const parent = node.parent
				if (parent?.type !== 'CallExpression' || parent.callee !== node) return
				if (FOCUSED_TESTS.has(node.name)) context.report({ node, message: ONLY_MESSAGE })
				else if (SKIPPED_TESTS.has(node.name)) skipped(node)
			},
			MemberExpression(node) {
				const property = keyName(node.property, node.computed)
				const root = calleePath(node.object)?.[0]
				if (!root || !TESTS.has(root)) return
				if (property === 'only') context.report({ node, message: ONLY_MESSAGE })
				else if (property === 'skip' || property === 'todo') skipped(node)
			},
		}
	},
})

const noWaitInTests = defineRule({
	meta: {
		type: 'problem',
		docs: { description: 'Disallow sleeps, timer waits and polling loops in tests.' },
	},
	create(context) {
		const timerPromiseNames = new Set<string>()
		const timerPromiseModules = new Set<string>()

		const insidePromiseExecutor = (node: Node) => {
			for (let current = node.parent; current; current = current.parent) {
				if (!FUNCTIONS.has(current.type)) continue
				const parent = current.parent
				return (
					parent?.type === 'NewExpression' &&
					parent.arguments[0] === current &&
					calleePath(parent.callee)?.join('.') === 'Promise'
				)
			}
			return false
		}

		const isWait = (node: Node): boolean => {
			if (node.type !== 'CallExpression') return false
			const path = calleePath(node.callee)
			if (!path) return false
			const last = path.at(-1)!
			if (last === 'sleep') return true
			if (path.length === 1) {
				if (timerPromiseNames.has(last)) return true
				return TIMERS.has(last) && insidePromiseExecutor(node)
			}
			return path.length === 2 && timerPromiseModules.has(path[0]!) && TIMERS.has(last)
		}

		// `await Promise.resolve()` and the like: a turn of the event loop, which a poll spins on.
		const isTick = (node: Node): boolean => {
			if (node.type !== 'AwaitExpression') return false
			const argument = unwrap(node.argument)
			if (argument?.type === 'CallExpression') {
				return calleePath(argument.callee)?.join('.') === 'Promise.resolve'
			}
			if (
				argument?.type !== 'NewExpression' ||
				calleePath(argument.callee)?.join('.') !== 'Promise'
			) {
				return false
			}
			// `new Promise(setImmediate)`, or an executor that schedules a timer.
			const [executor] = argument.arguments
			if (executor?.type === 'Identifier') return TIMERS.has(executor.name)
			return (
				(executor?.type === 'ArrowFunctionExpression' || executor?.type === 'FunctionExpression') &&
				some(executor.body, isWait)
			)
		}

		// A loop polls when something it reads, not a counter it advances, decides when it stops.
		const polls = (
			loop: ESTree.DoWhileStatement | ESTree.ForStatement | ESTree.WhileStatement,
		): boolean => {
			if (!some(loop.body, isTick)) return false
			const exits = some(
				loop.body,
				(node) =>
					node.type === 'IfStatement' &&
					some([node.consequent, node.alternate], (child) =>
						['BreakStatement', 'ReturnStatement', 'ThrowStatement'].includes(child.type),
					),
			)
			if (exits) return true
			const test = unwrap(loop.test)
			if (!test || (test.type === 'Literal' && test.value === true)) return false
			if (test.type !== 'BinaryExpression' || !['<', '<=', '>', '>='].includes(test.operator)) {
				return true
			}
			const counters = [test.left, test.right].flatMap((side) => {
				const name = unwrap(side)
				return name?.type === 'Identifier' ? [name.name] : []
			})
			const advanced = some(
				[loop.body, loop.type === 'ForStatement' ? loop.update : null],
				(node) => {
					const target =
						node.type === 'UpdateExpression'
							? unwrap(node.argument)
							: node.type === 'AssignmentExpression' && ['+=', '-='].includes(node.operator)
								? unwrap(node.left)
								: undefined
					return target?.type === 'Identifier' && counters.includes(target.name)
				},
			)
			return !advanced
		}

		const checkLoop = (
			node: ESTree.DoWhileStatement | ESTree.ForStatement | ESTree.WhileStatement,
		) => {
			if (polls(node)) context.report({ node, message: WAIT_MESSAGE })
		}

		return {
			ImportDeclaration(node) {
				if (!TIMER_PROMISES.has(node.source.value)) return
				for (const specifier of node.specifiers) {
					if (specifier.type === 'ImportNamespaceSpecifier') {
						timerPromiseModules.add(specifier.local.name)
					} else if (specifier.type === 'ImportSpecifier') {
						const imported = keyName(specifier.imported)
						if (imported && TIMERS.has(imported)) timerPromiseNames.add(specifier.local.name)
					}
				}
			},
			CallExpression(node) {
				if (isWait(node)) context.report({ node, message: WAIT_MESSAGE })
			},
			DoWhileStatement: checkLoop,
			ForStatement: checkLoop,
			WhileStatement: checkLoop,
		}
	},
})

export default definePlugin({
	meta: { name: 'amberite' },
	rules: {
		'no-test-timeout-or-unquarantined-skip': noTestTimeoutOrUnquarantinedSkip,
		'no-wait-in-tests': noWaitInTests,
	},
})
