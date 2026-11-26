import { defineConfig } from '@coderabbitai/config'

// Each doc is the rule for the code it names: review holds a change to the doc its author read.
const DOC_RULES = [
	{
		files: 'docs/internals/fixtures.md',
		applyTo: 'packages/fixtures/**/*.ts',
		instructions: 'Hold fixtures and their helpers to the rules in docs/internals/fixtures.md.',
	},
	{
		files: 'docs/internals/scripts.md',
		applyTo: 'scripts/**/*.ts',
		instructions:
			'Hold repository scripts and the dev runner to docs/internals/scripts.md, including process ownership: nothing kills a process by name or path.',
	},
	{
		files: 'docs/internals/testing.md',
		applyTo: '{apps,packages,scripts}/**/*.{ts,mjs,rs,vue}',
		instructions:
			'Hold changed tests to the rules in docs/internals/testing.md. Request tests only as those rules allow.',
	},
	{
		files: 'AGENTS.md',
		applyTo: 'docs/internals/**/*.md',
		instructions:
			'Hold internal doc edits to the Documentation rules in AGENTS.md: keep only durable architectural decisions, constraints that span components, and cross-cutting traps. Flag single-feature walkthroughs, field/method catalogs, narrated control flow, and PR appends.',
	},
] as const

export default defineConfig({
	reviews: {
		// Our agents read the finding and fix it. CodeRabbit's own fixer and its ready-made prompt stay off.
		enable_prompt_for_ai_agents: false,
		finishing_touches: { autofix: { enabled: false } },
		path_instructions: DOC_RULES.map((rule) => ({
			path: rule.applyTo,
			instructions: rule.instructions,
		})),
	},
	knowledge_base: {
		code_guidelines: {
			filePatterns: DOC_RULES.map(({ files, applyTo }) => ({ files, applyTo })),
		},
	},
})
