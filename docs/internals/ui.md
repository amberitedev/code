# UI work

How the App's UI should behave. The work itself lives in Linear (see `docs/agents/issue-tracker.md`).
Terms come from `CONTEXT.md`.

## How the UI behaves

- **One library.** Every entry is an instance. An instance with a server is shown as a server.
- **One server per instance.** A second server with the same content is a duplicated instance.
- **The instance is the frame.** Server features live inside the instance's existing tabs, never in
  a second place. One tab list per page; no tabs inside tabs.
- **Same screen for everyone.** A friend you shared with sees the owner's screens with owner-only
  controls hidden, not a different page.
- **Ask in the App, at the moment it matters.** Never send a user to the command line to finish
  something (the EULA is asked on first start, for example).
- **Every feature has a way in, a way out, and a way to see its state.**
- **Technical things sit behind Advanced.** A non-technical user never needs them.
- **Keep upstream Modrinth UI** unless Amberite has a reason to differ. Upstream hosting UI with no
  Core route behind it is hidden, unless the route is small and the feature is worth it.
- **Look:** dark, true black (`#000`) background, white text, information dense, minimal copy. No
  decorative cards or pills, no gray subtitle lines above sections, no em dashes, no continuously
  repainting animations.
