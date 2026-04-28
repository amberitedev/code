# Design Inspector Overview

## The Problem

The user has a Vue application. They see something in the UI that needs changing. They want to tell the AI "make this sidebar wider" or "fix the spacing in this header" without manually describing which file to edit or which component to modify. They want to click the element, type a comment, and have the AI understand exactly what they're referring to.

## The Solution

Vue-devtools already has an element picker (Alt+click). We extend it to add a comment input. When the user submits their comment, Vue-devtools sends it plus metadata (component name, source file, HTML, CSS classes) to OpenCode. OpenCode routes it to the active session and displays it as a chip above the prompt input. When the user sends their message, the chip content becomes context for the AI.

## How Routing Works

Vue-devtools does NOT know which session to send to. It POSTs blindly. OpenCode maintains a registry of which session is currently active for each project. When Vue-devtools sends a comment, OpenCode looks up the registry and routes the comment to that session. This simplifies Vue-devtools and centralizes routing logic in OpenCode.

## Comment States

Each comment exists in one of five states:

**Embedded** — The comment appears as a chip above the prompt input. It will be sent to the AI with the next message. After sending, it becomes archived.

**Suspended** — The comment was embedded but the user switched to a different mode. It's hidden but preserved. When the user returns to the original mode, it automatically re-embeds. This state exists so users don't lose their comments when switching contexts.

**Unembedded** — The user clicked the X button on a chip. The comment moved to the comments panel. It will NOT automatically re-embed. The user must click it in the panel to embed it. This gives the user control over which comments they want active.

**Archived** — The comment was already sent to the AI. It's a historical record. The user can view archived comments if they enable the "show archived" toggle. They can delete archived comments to clean up.

**Pending** — Vue-devtools sent a comment but no session was active. The comment sits in a cache. When a session activates, it gets processed (embedded or unembedded depending on settings). This is rare because users typically have a session active when using Vue-devtools.

## State Transitions

When Vue-devtools sends a comment:
- If session active → comment becomes embedded (or unembedded if auto-embed disabled)
- If no session → comment becomes pending

When user switches modes:
- Embedded comments of the OTHER mode → suspended
- Suspended comments of the CURRENT mode → embedded

When user clicks X on chip:
- Embedded → unembedded

When user clicks comment in panel:
- Unembedded → embedded

When user sends message:
- All embedded → archived

When user deletes archived:
- Archived → removed from system

## Modes

OpenCode has three agent modes: design, build, and plan.

Design mode handles comments from Vue-devtools. These are called "design comments." When in design mode, the user sees design comments in the panel and as chips.

Build and plan modes handle "line comments" — the existing system where users comment on specific lines of code in the editor. When in build or plan mode, the user sees line comments.

Switching between modes triggers the suspend/resume behavior. Design comments suspend when leaving design mode. Line comments suspend when leaving build/plan mode. Suspended comments resume when returning to their mode.

## The Auto-Embed Behavior

By default, when a comment arrives from Vue-devtools:
1. The system auto-switches to design mode
2. The comment auto-embeds and appears as a chip

This is the happy path. The user clicks, comments, and immediately sees the chip ready to send.

A setting lets users disable auto-embed. When disabled, comments go to unembedded state and appear in the panel. The user manually embeds them by clicking.

## The Full User Flow

1. User holds Alt and clicks an element in their Vue app
2. Vue-devtools shows the element info and a comment input
3. User types a comment (can use @file or #component syntax)
4. User hits enter, Vue-devtools POSTs to OpenCode
5. OpenCode routes to active session, embeds the comment
6. Frontend receives SSE event, auto-switches to design mode
7. User sees the chip above the prompt input
8. User types their main message and hits send
9. Embedded comments become context for AI, then archived
10. AI reads the component files and makes targeted edits