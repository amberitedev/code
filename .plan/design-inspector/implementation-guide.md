# Design Inspector Implementation Guide

This document explains everything discussed about the design inspector feature. It covers all systems, flows, behaviors, and design decisions in plain English without code.

---

## The Goal

The user wants to click an element in their Vue application, type a comment about it, and have that comment plus the element's metadata sent to OpenCode where the AI can see it and make changes. The Vue-devtools extension already has an element picker (Alt+click). We extend it to add a comment input and send the data to OpenCode. OpenCode receives it, routes it to the correct session, and displays it as a chip that the user can see before sending their message to the AI.

---

## The Mental Model

Think of this as a bridge between two worlds. The Vue-devtools world knows about Vue components, their source files, their HTML structure, and their CSS classes. The OpenCode world knows about sessions, AI agents, and the prompt input. The bridge sends component information from Vue-devtools to OpenCode, where it becomes context for the AI.

The key insight the user emphasized repeatedly: Vue-devtools does NOT need to know which session it's sending to. It just sends blindly. OpenCode handles the routing because OpenCode knows what session is currently active. This simplifies the Vue-devtools side dramatically.

---

## Comment States Explained

Each comment can be in one of five states. The state determines where the comment appears and what happens to it when the user interacts with it.

**Embedded** means the comment is shown as a chip above the prompt input. These chips are the things the user sees right before they send their message. When they hit send, all embedded comments get packaged up and sent to the AI as context. After sending, embedded comments become archived.

**Suspended** is a special state that only happens when the user switches between modes. The system has three modes: design, build, and plan. Design mode handles design comments (from Vue-devtools). Build and plan mode handle line comments (from code reviews). When the user switches from design to build, all their embedded design comments get suspended. They disappear from the chips area but they're not gone. When the user switches back to design mode, those suspended comments automatically re-embed themselves. This means the user doesn't lose their work when switching modes. The suspended state exists to preserve embedded comments across mode switches.

**Unembedded** is what happens when the user manually clicks the X button on a chip. The comment goes into the comments panel but it won't automatically re-embed. The user has to click it in the panel to embed it again. This gives the user control. They can say "I don't want this comment right now but I might want it later" without losing it entirely.

**Archived** is the state after a comment has been sent to the AI. It's a historical record. The user can see archived comments if they enable the "show archived" toggle. They can also delete archived comments if they don't want them cluttering up the panel.

**Pending** is a temporary state that only exists when Vue-devtools sends a comment but no session is active yet. Imagine the user has OpenCode open but they haven't navigated to a session page. Vue-devtools sends a comment. OpenCode stores it in a pending cache. When the user opens a session, that pending comment gets processed. If auto-embed is enabled, it becomes embedded. If auto-embed is disabled, it becomes unembedded. In normal use, this state is rare because the user usually has a session active when they're using Vue-devtools.

---

## Modes Explained

The system has three agent modes: design, build, and plan. Each mode has its own set of comments.

Design mode is for UI/design work. It handles comments that came from Vue-devtools. When the user is in design mode, they see design comments. Any suspended design comments automatically re-embed. Any embedded line comments (from build/plan mode) get suspended.

Build and plan mode are for code work. They handle line comments, which are the existing comment system in OpenCode where users can comment on specific lines of code. When the user is in build or plan mode, they see line comments. Suspended line comments re-embed. Embedded design comments get suspended.

The user mentioned that design might be a "mode" rather than an "agent." The distinction matters for how the system prompts the AI. A mode might have special prompting behavior that injects context differently. The user said the design mode should prompt the AI to first read all the relevant files from the design comments before making any edits. This ensures the AI has full context about the components before touching them.

---

## The Flow When Vue-Devtools Sends a Comment

The user holds Alt and clicks an element in their Vue app. Vue-devtools shows the element info in its panel. The user types a comment in a text input. They might use @filename syntax to mention a file, or #component syntax to mention a Tailwind component from packages/ui. They hit enter.

Vue-devtools constructs a comment object containing: the component name, the source file path, the HTML of the element, the CSS classes, and the user's comment text. It POSTs this to OpenCode at a simple endpoint. No session ID in the URL. Vue-devtools doesn't know about sessions.

OpenCode receives the POST. It looks up which session is currently active for this project directory. If a session is active, the comment gets embedded immediately. If no session is active, the comment goes into the pending cache.

OpenCode publishes a Bus event to notify the frontend. The frontend, which is subscribed to SSE events, receives this event. It knows a new design comment arrived.

If auto-embed is enabled (the default), the frontend automatically switches the agent to design mode. The comment appears as a chip above the prompt input. The user sees it and knows it will be sent with their next message.

If auto-embed is disabled, the frontend doesn't auto-switch. The comment sits in the panel in an unembedded state. The user can click it to embed it manually.

---

## The Flow When User Sends a Message

The user types their message and hits send. Before building the request, the frontend takes all embedded design comments. It constructs context parts for each one: the component name, the source file, the HTML, the CSS classes, and the user's comment. These get injected into the AI prompt.

The embedded comments then move to the archived state. They're now a historical record.

The AI receives the message plus all the design comment context. The design agent's system prompt tells it to first read all the source files mentioned in the design comments. It reads them, understands the component structure, and then makes targeted edits based on the user's comments.

---

## The Flow When User Switches Modes

The user is in design mode with three embedded design comments. They switch to build mode to do some coding work.

The frontend calls the backend to suspend all design-mode comments. The three embedded comments become suspended. They disappear from the chips area.

If the user had any suspended build-mode comments from earlier, those resume. They become embedded and appear as chips.

When the user switches back to design mode, the reverse happens. Build comments get suspended. Design comments resume and re-embed.

This mode-switching behavior ensures the user always sees the relevant comments for their current work. They don't have to manually manage comments when switching contexts.

---

## The Comments Panel Explained

The comments panel is where users see all their comments. It replaces the existing comments panel location but with new functionality.

The panel has a toggle switch. When the toggle is OFF, the panel shows only comments for the current mode. If the user is in design mode, they see design comments. If they're in build mode, they see line comments. This keeps the view focused.

When the toggle is ON, the panel shows everything with headers. There's a Design header with all design comments below it. Then a Line header with all line comments below it. The user can scroll through both sections. This lets the user see their full comment inventory.

At the bottom, there's an Archived section that's hidden by default. The user can enable "show archived" to see comments they've already sent. Each archived comment has a delete button. This lets the user clean up old comments they don't need anymore.

Each comment appears as a bubble. The user described it as "like a chat" — bubbles stacked vertically, each bubble showing the comment text and metadata like the component name and file path. The bubble grows vertically to fit the content but has a fixed width. It fills the panel width. This matches the style of user messages in OpenCode's message timeline.

The panel scrolls independently like the file tree panel. The user can only scroll it when their mouse is hovering over it.

---

## The Chip Behavior Explained

Embedded comments appear as chips above the prompt input. These are small rectangles that show the component name and a truncated version of the comment text. They have an X button.

When the user clicks the X, the chip disappears. The comment moves to unembedded state. It now appears in the comments panel. The user can click it in the panel to re-embed it.

Chips are mode-specific. If the user is in design mode, only design chips appear. If embedded line comments exist, they're suspended and don't show as chips. This keeps the chip area clean and relevant to the current task.

---

## The Pending Cache Explained

The pending cache handles the edge case where Vue-devtools sends a comment but no session is active. This might happen if the user has OpenCode open on the workspace page (no session) and they click an element in Vue-devtools.

The backend stores the comment in a cache keyed by project directory. When the user navigates to a session, the frontend sends an activation notification to the backend. The backend processes any pending comments for that directory. It either embeds them (if auto-embed enabled) or leaves them unembedded (if disabled).

The user said this is a rare case. Most of the time, the user has a session active when using Vue-devtools. But the pending cache ensures comments aren't lost if the timing is off.

---

## The Active Session Registry Explained

The backend needs to know which session is currently active so it can route incoming Vue-devtools comments. The frontend tells it via a simple POST endpoint.

When the user navigates to a session page in OpenCode, the frontend calls POST /session/activate with the session ID. The backend stores this in a registry keyed by project directory. When Vue-devtools POSTs a comment, the backend looks up the registry, finds the active session, and routes the comment there.

The registry is per-project. If the user has multiple projects open, each has its own active session. Vue-devtools comments go to the active session for their project.

---

## The Autocomplete Feature Explained

The comment input in Vue-devtools supports special syntax for referencing files and components. The @ symbol triggers file autocomplete. The # symbol triggers component autocomplete.

The user types @ and Vue-devtools requests a list of files from OpenCode. OpenCode returns file paths from the project. Vue-devtools shows them in a dropdown. The user picks one.

Similarly, # triggers a request for components. OpenCode scans packages/ui (where the Tailwind components live) and returns a list of component names. Vue-devtools shows them. The user picks one.

This autocomplete is important because it lets the user reference specific files and components in their comment. The AI can then read those exact files. The user said this autocomplete should come from OpenCode, not be built into Vue-devtools. OpenCode already has file tree knowledge, so it can provide the data.

---

## The Auto-Embed Setting Explained

By default, when Vue-devtools sends a comment, it auto-embeds and auto-switches to design mode. This is the "happy path" — the user clicks, comments, and immediately sees the chip ready to send.

But the user might want different behavior. They might want comments to appear in the panel without auto-embedding. They might not want to auto-switch to design mode. A setting lets them disable auto-embed.

When auto-embed is disabled, incoming comments go to unembedded state. They appear in the panel. The user manually clicks them to embed. The system doesn't auto-switch the agent.

---

## The Design Agent Prompting Explained

The user wants the design agent to have special prompting behavior. When design comments are embedded and the user sends a message, the AI should first read all the source files mentioned in those comments before making any edits.

This ensures the AI has full context. It doesn't just blindly edit based on the comment text. It reads the actual component code, understands the structure, and then makes informed changes.

The design agent might be a hardcoded native agent rather than a markdown file. The user mentioned this gives more control over system prompts. The native agent can inject prompts that tell the AI to read files first.

---

## Summary of Key Decisions

Vue-devtools sends blindly without knowing the session. OpenCode routes based on active session registry.

Comments have five states: embedded, suspended, unembedded, archived, pending.

Modes switch between design and build/plan. Embedded comments of the other mode get suspended. Suspended comments of the current mode get re-embedded.

The pending cache handles comments that arrive before a session is active.

The comments panel shows suspended and unembedded comments for the current mode. Toggle ON shows all modes with headers. Archived comments can be deleted.

Auto-embed is the default behavior. Setting to disable exists.

Design agent prompts AI to read source files before editing.

Autocomplete for @files and #components comes from OpenCode endpoints.

---

## What the User Emphasized Most

The user repeated several points multiple times because they were important:

Vue-devtools does NOT need session knowledge. OpenCode handles routing.

The pending state is temporary and rare. Comments usually go straight to embedded.

When switching modes, embedded comments suspend, suspended comments resume.

Unembedded means the user pressed X. It won't auto-embed.

Suspended means auto-unembedded by mode switch. It will auto-embed when returning.

Archived comments can be deleted.

The comments panel uses bubbles like chat messages, fixed width, grows vertically.

All three embedded comments become chips when they arrive.

Auto-embed enabled means auto-switch to design AND auto-embed.

No active session means store in cache until session activates.