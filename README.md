# Amberite

Amberite is a Minecraft launcher for modding and sharing, with self-hosted servers. Run the Rust
server (Core, shipped as Copal) on any machine and manage it from the desktop App. Sharing is not
tied to a Core and works for everyone.

## Running it

Install Node 24.15 or newer and [Vite+](https://viteplus.dev/guide/) (the `vp` command) as
[docs/internals/scripts.md](docs/internals/scripts.md#first-checkout) describes, then:

```sh
vp i
vp run dev
```

`vp run dev` starts the backend, a Core and the App.

## Docs

[AGENTS.md](AGENTS.md) explains how the repo is laid out and how to work in it.
[docs/internals](docs/internals) holds architectural decisions and the constraints behind them.

## Credit

Amberite is built on [Modrinth](https://github.com/modrinth/code). Most of the App and its UI come
from Modrinth's launcher, and Modrinth handles public content.

## License

Each package has its own license. See [COPYING.md](COPYING.md), including its rules on Modrinth
branding.
