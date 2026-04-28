# Vue-DevTools Side Implementation

## What Vue-DevTools Already Has

Vue-devtools is a browser devtools extension. It runs in the user's Vue app context (like localhost:5173). It already has:

**Component tree** — Shows all Vue components with their names and hierarchy.

**Element picker** — Hold Alt and click any element. Vue-devtools highlights it and shows component info in the devtools panel. It reads the __file property from Vue components (requires the vue-devtools Vite plugin to be installed in the user's project).

**__vueParentComponent access** — Vue-devtools can traverse the Vue instance hierarchy to find parent components.

## What Vue-DevTools Needs to Add

### Comment Input UI

When an element is selected via Alt+click, Vue-devtools should show a comment input. This is a text bubble that grows vertically as the user types. It appears in the devtools panel.

The input should support:
- Regular text typing
- @filename syntax for referencing files
- #component syntax for referencing Tailwind components from packages/ui

When the user types @, an autocomplete dropdown appears. Vue-devtools requests files from OpenCode. Similarly, # triggers component autocomplete.

The input has a submit button or responds to Enter key. On submit, Vue-devtools sends the comment to OpenCode.

### Comment Object Structure

When submitting, Vue-devtools constructs an object with:

```json
{
  "id": "unique-id",
  "component": "Sidebar",
  "source_file": "src/components/Sidebar.vue",
  "source_lines": [45, 60],
  "html": "<div class=\"sidebar w-64\">...</div>",
  "css_classes": ["sidebar", "w-64", "bg-gray-800"],
  "parent": { "component": "AppLayout", "file": "src/layouts/AppLayout.vue" },
  "comment": "Make this sidebar wider"
}
```

The id is generated client-side. The component name comes from the Vue instance. The source file comes from __file property. The HTML comes from element.outerHTML (truncated if too long). CSS classes come from element.classList. Parent info comes from traversing __vueParentComponent.parent. Source lines would require extra work (optional for MVP).

### POST to OpenCode

Vue-devtools POSTs this object to OpenCode at:

POST http://localhost:4096/design-comments

The URL is configurable. Default is localhost:4096. Vue-devtools does NOT include a session ID. OpenCode handles routing.

The POST is simple. Vue-devtools doesn't wait for any response beyond success/error. If the POST fails, Vue-devtools could show a brief error toast.

### Autocomplete Request Flow

When the user types @ in the comment input:

1. Vue-devtools detects the @ character
2. Vue-devtools calls GET http://localhost:4096/files/autocomplete?q=partial-path
3. OpenCode returns an array of file paths matching the query
4. Vue-devtools shows them in a dropdown below the input
5. User clicks one to insert it into the comment

Same flow for # but calls /components/autocomplete instead.

The autocomplete endpoint scans the project for matching files/components. Vue-devtools doesn't need to know the file system. OpenCode already has that knowledge.

## What Vue-DevTools Does NOT Need

Vue-devtools does NOT need:
- Session ID knowledge
- Project directory knowledge
- WebSocket or SSE connection
- Persistent storage of comments
- Any knowledge of OpenCode's internal state

All routing and state management happens in OpenCode. Vue-devtools just sends and forgets.

## The Element Picker Interaction

The existing Alt+click picker shows component info. Vue-devtools should add the comment input below that info. The flow:

1. User holds Alt and clicks an element
2. Vue-devtools highlights the element in the page
3. Vue-devtools panel shows component details (name, file, props, etc.)
4. Vue-devtools panel also shows the comment input
5. User types comment and hits Enter
6. Vue-devtools sends to OpenCode, shows brief success indicator
7. Comment input clears, ready for next comment

The user can send multiple comments in sequence. Each one POSTs independently. OpenCode will embed all of them.

## Configuration

Vue-devtools might need a config panel to set:
- OpenCode URL (default localhost:4096)
- Maybe other options in the future

For MVP, hardcoded localhost:4096 is fine.

## Summary

Vue-devtools adds three things:
1. Comment input UI with @ and # autocomplete
2. POST client to send comments to OpenCode
3. Minimal config for OpenCode URL

Everything else — routing, state management, session tracking, chip rendering — is handled by OpenCode. Vue-devtools is a thin client that sends and forgets.