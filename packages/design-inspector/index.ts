// NOTE: designInspectorVitePlugin is intentionally NOT re-exported here.
// It imports node:url and vite-plugin-vue-inspector (Node.js-only modules)
// which crash the browser when this barrel is imported by the Vue app.
// Import it directly from './vite-plugin.js' in vite.config.ts instead.
export { DesignInspectorPlugin } from './vue-plugin.ts'
