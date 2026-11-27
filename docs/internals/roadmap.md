# Roadmap

> For maintainers and agents.

What is left before Amberite is released. Everything here is what we want, stated as fact. When
something stops being true, change it. When an item is built, delete it. There are two
milestones: the **alpha** is the first build other people install, and it goes to friends. The
**release** is when we announce it.

## What Amberite is when it ships

- **Accounts.** Signing in with Minecraft is signing in to Amberite, and the first sign-in creates
  the account. More Minecraft accounts link to it in settings, and the first sign-in offers the
  ones already in the launcher. A Minecraft account belongs to one Amberite account and is never
  merged into another. There is no password, no email and no recovery.
- **Names.** One linked account is your main, and you choose which. Your username is its
  Minecraft name. Your display name is yours to set and need not be unique. Search finds you by
  the name of any linked account.
- **Proof.** The backend learns you own a Minecraft account through Minecraft's own join
  handshake, so your token never leaves your computer. We sign in to Microsoft with the same
  client id upstream uses.
- **Instances and servers.** You build an instance and create servers from it. A server follows
  its instance, and one instance can have many. A server made without an instance is its own
  source.
- **Sharing.** Sharing an instance gives a friend the mods. Sharing a server gives them the mods
  and a place to play, never edit rights. Sharing one does not share the other. There is one
  share per thing, with one writer.
- **No roles.** One person owns a server and nobody co-manages. A Core answers its owner only.
  Friends read what was shared with them from the backend and from other Cores.
- **Library.** Two pages, as upstream has them: Library for packs, Servers for the servers you
  host. A server shared with you lives inside its pack, and a pack is installed once however many
  of its servers you were given.
- **Whitelist.** New servers have it on. Sharing a server adds the friend's Minecraft accounts,
  and unsharing removes them.
- **Cores.** A Core runs on a machine you own, a home server first. One command installs it
  there. On your own PC the App sets it up with no terminal. A Core signs in with a code approved
  in the App.
- **Joining.** Friends join a server with no port forwarded. The two computers connect directly,
  and the backend only introduces them. Two strict routers cannot connect, and the App says so.
- **Website.** Static: landing page, downloads, links that open the App, terms and privacy.
  Nothing on it needs an account.
- **Look.** Modrinth's layout and orange stay. None of Modrinth's branding does. The word
  "Modrinth" stays only where it names where content comes from.
- **Platforms.** Windows and Linux, unsigned, so Windows shows a warning on install.
- **Cost.** The backend stays at $0. Shared files and game traffic never pass through it.
- **Code.** Built on upstream. A screen that exists for both an instance and a server has one
  implementation. Every endpoint is declared in contracts.

## To build

Each group is about one chat.

**Accounts**

- Minecraft sign-in on the backend and in the App, replacing email and password.
- Linking and unlinking Minecraft accounts, the main account, display name, and search.
- A Core learns its players' Minecraft accounts, for the whitelist.

**Servers and sharing**

- A friend given two servers on one pack reaches both, and the second invite does not ask to
  install the pack again (#132).
- Whitelist on share (#119).
- Leaving a server in one step. An invite names the server, not the pack.
- No dead controls: every control works or says why it cannot.
- An instance and a server each say whether they are private or shared, and with how many.
- Where a linked server's history lives (#131, #133), storage counted across your shares (#122),
  more friction before deleting a server (#124).

**Hosting on your own machine**

- The Core's first release, with an install command for Windows as well as Linux.
- "Set up a Core on this PC" in the App: download, install and pair, with nothing to type. This
  is the feature that has to feel effortless.
- A server behind a router that blocks ports gets a plain explanation and the fix.

**Translations**

- Translations of our own text. Upstream's arrive through merges. Text we add or change needs its
  own, kept apart from upstream's files so merges stay clean, and the product name is ours in
  every language.

**Identity**

- One folder of Amberite brand assets replaces every Modrinth icon, logo and illustration. A
  placeholder mark is enough for the alpha.
- Stop being Modrinth to other services: the Discord application id and the `modrinth://` link
  scheme.

**Production**

- Deploy the backend on our domain, check what its cost does as users grow, and build its
  address into release builds.
- The App's release pipeline: our own update feed and key, Windows and Linux builds.
- The website.
- Licence notices, a link to the source, and "not an official Minecraft product".
- Sync upstream, then freeze the data: clean schemas, a final app identifier and data folder.
  From the alpha on, changes migrate.

**Alpha**

- One full pass in the real App: install, play, host, share, join, update, leave, then again with
  the Core offline and with the backend offline.
- ilai's friends, on their own machines and routers.

**Release**

- Playing without port forwarding: the backend introduces a friend's App to the Core, as it
  already does the owner's, and Minecraft travels over that direct connection. There is no
  relay, so game traffic never reaches the backend. The plan is `.plans/connect.md`.
- A real logo, and a look over every screen.
- A video, and posts where modded players and self-hosters are.

## Only ilai

- Buy the domain, and put a card on Cloudflare.
- Terms and a privacy policy.
- A Discord application, and the Core's release signing key moved into a password manager.

## Order

- **Now, side by side:** accounts, the two-servers fix, identity, the Core's first release, the
  App's release pipeline, translations.
- **After the Core's first release:** setting up a Core from the App.
- **After accounts:** the whitelist.
- **After the domain and the card:** deploying the backend, the website.
- **Last before the alpha:** the freeze, the full pass, friends.
- **Between the alpha and the release:** playing without port forwarding.

## Later

- Collaboration, designed from scratch and not as Modrinth's access page.
- One library, where a server and its pack are one card.
- Account recovery.
- Signing in on the website, and managing servers from it.
- Sharing a singleplayer world with friends, over the same tunnel.
- Free Windows signing for open-source projects, and macOS when someone can test it.
- Our own Microsoft client id, if upstream's stops working.
- Memory in one place (#125), CurseForge packs on servers (#126), crash logs that name the mod
  (#129), linked servers that follow offline (#134).
