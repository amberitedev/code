# providers/

Boot-time dependency injection. Called once from `App.vue` to wire Vue `provide/inject` pairs that pages and components consume.

---

## Structure

```
setup.ts              Entry point — call setupProviders() at app boot
app-notifications.ts  AbstractWebNotificationManager implementation
app-popup-notifications.ts  AbstractPopupNotificationManager implementation
content-install.ts    Content install state provider
download-progress.ts  Download progress state provider
instance-settings.ts  Instance settings modal provider
server-install.ts     Server install flow provider

setup/                Individual provider setup functions
  auth.ts             Auth state injection
  creation-modal.ts   Instance/server creation modal
  file-picker.ts      Native file picker bridge
  instance-import.ts  Modpack/instance import flow
  loading-state.ts    Global loading bar state
  server-install-content.ts  Server content install steps
  tags.ts             Modrinth tags preload
```

---

## How It Works

`setupProviders()` in `setup.ts` is the single entry point. It calls each `setup/` function, passing the notification managers. Each setup function calls Vue's `provide()` so downstream components can `inject()` the result without prop-drilling.

The notification managers (`app-notifications.ts`, `app-popup-notifications.ts`) implement abstract interfaces from `@modrinth/ui` and are passed into `setupProviders()` by the caller.

---

## Gotchas

- All `provide()` calls must happen in a Vue component setup context — `setupProviders()` is called from `App.vue`'s `<script setup>`, not from outside a component.
- Adding a new globally-injectable thing: create a file in `setup/`, call it from `setup.ts`.
