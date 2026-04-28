# OpenCode Side Implementation

## Backend Systems

### Active Session Registry

OpenCode needs to know which session is currently active for each project. This registry is a simple mapping from project directory to session ID. The frontend updates it by calling an activation endpoint whenever the user navigates to a session page. When Vue-devtools POSTs a comment, the backend looks up this registry and routes the comment to the correct session.

The registry handles the case where no session is active. If Vue-devtools sends a comment but the registry has no entry for that project, the backend stores the comment in a pending cache instead of routing it.

### Pending Cache

This is a temporary holding area for comments that arrive before a session is active. It's keyed by project directory. When the frontend activates a session, the backend processes any pending comments for that directory. Depending on the auto-embed setting, they either become embedded immediately or go to unembedded state.

The pending cache is rarely used because users normally have a session active when clicking elements in Vue-devtools. But it prevents comments from being lost if the timing is wrong.

### Comment Store

Each session has its own comment store with five separate collections:

**Embedded collection** — Comments that are chips above the input. These are the active comments that will be sent with the next message.

**Suspended collection** — Comments that were embedded but got suspended due to mode switch. They're preserved here and will re-embed when the user returns to their mode.

**Unembedded collection** — Comments the user manually removed by clicking X. They stay here until the user clicks them in the panel to re-embed.

**Archived collection** — Comments that were already sent. They're a historical record. Users can view and delete them.

**The store tracks the mode for each comment** — either "design" or "line". This determines which mode the comment belongs to and when it should suspend/resume.

### Bus Events

The backend uses an event bus to notify the frontend when comments arrive. Two events matter:

**design_comment.added** — Published when a comment gets embedded because a session was active. The frontend receives this via SSE and knows to update the UI.

**design_comment.pending_resolved** — Published when a pending comment gets processed because a session just activated. The frontend knows comments that were waiting are now available.

### Endpoints

The backend exposes several endpoints for the frontend and Vue-devtools:

**POST /session/activate** — Frontend calls this when navigating to a session. Tells the backend which session is active. Backend processes any pending cache for that project.

**POST /design-comments** — Vue-devtools calls this with no session ID. Backend routes to active session via registry.

**POST /session/:id/design-comments/embed** — Frontend calls to embed a comment (when user clicks in panel).

**POST /session/:id/design-comments/unembed** — Frontend calls to unembed a comment (when user clicks X on chip).

**POST /session/:id/design-comments/suspend** — Frontend calls to suspend comments of a specific mode (when switching away from that mode).

**POST /session/:id/design-comments/resume** — Frontend calls to resume suspended comments of a specific mode (when switching to that mode).

**POST /session/:id/design-comments/take** — Frontend calls when sending message. Returns all embedded comments and moves them to archived.

**GET /session/:id/design-comments** — Returns all comments for the session grouped by state and mode.

**DELETE /session/:id/design-comments/archived/:commentId** — Deletes an archived comment.

### Autocomplete Endpoints

Vue-devtools needs to know what files and components exist for autocomplete:

**GET /files/autocomplete?q=query** — Returns matching file paths from the project.

**GET /components/autocomplete?q=query** — Returns component names from packages/ui.

## Frontend Systems

### Session Activation

When the user navigates to a session page (params.id changes), the frontend calls the activation endpoint. This happens in a reactive effect that watches the route params. The call is simple and fire-and-forget.

### SSE Listener

The frontend subscribes to the SSE event stream. When it receives a design_comment.added event, it:
- Checks if auto-embed is enabled
- If enabled, switches to design agent and refreshes the comments
- If disabled, just refreshes the comments (comment will be in unembedded state)

### Comments Context

A SolidJS context that manages the comment state for the current session. It fetches from the backend, provides methods to embed/unembed, and tracks the current state.

The context exposes:
- Comments grouped by state and mode
- Methods to embed, unembed, take, delete
- A refresh method that fetches latest state from backend

### Comments Panel Component

The panel sits in the layout where the old comments panel was. It shows:

**Header with toggle** — A switch that toggles between "current mode only" and "show all modes."

**Current mode view (toggle OFF)** — Shows suspended and unembedded comments for the current agent mode. Each comment is a clickable bubble. Clicking an unembedded comment embeds it.

**All modes view (toggle ON)** — Shows a Design header section with design comments, then a Line header section with line comments. Both sections scroll independently.

**Archived section** — Hidden by default. Has its own toggle "show archived." Each archived comment has a delete button (trash icon). Clicking delete removes it permanently.

The panel uses chat-bubble styling. Each comment is a rounded rectangle with the comment text and metadata (component name, file). Bubbles stack vertically and fill the panel width. They grow vertically to fit content. This matches the user-message style in the timeline.

### Chips Above Input

Embedded comments render as chips in the prompt input area. A chip shows:
- Code icon
- Component name
- Truncated comment text (maybe 30 chars)
- X button to unembed

Chips are interactive. Clicking X calls the unembed endpoint and the chip disappears. The comment moves to the comments panel in unembedded state.

### Mode Switching Logic

When the user changes the agent (from design to build/plan or vice versa), the frontend:
- Calls suspend endpoint for the mode being left
- Calls resume endpoint for the mode being entered
- Refreshes the comments to show updated state

This happens in the agent switching code, not in a separate effect. It's part of the agent.set function.

### Submit Handler

When the user sends a message, before building the request:
- Call the take endpoint to get all embedded comments
- Move embedded comments to archived (backend does this)
- Build context parts from the comments (component name, file, HTML, CSS, comment)
- Inject those parts into the message context
- The AI receives them and reads the relevant files

### Settings

A setting for auto-embed behavior. Default is enabled. When disabled:
- Comments from Vue-devtools go to unembedded state
- No auto-switch to design agent
- User manually clicks in panel to embed