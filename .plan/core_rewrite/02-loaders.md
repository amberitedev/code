# Part 2 — Loader Support

Adds complete server JAR resolution for all 6 loaders.
All loader logic lives in `infrastructure/minecraft/flavours.rs`.

The current state: Vanilla and Fabric work. Quilt, Paper, Forge, and NeoForge return
`FlavourError::Unsupported`. Fabric and Vanilla are reference implementations to follow.

---

## How Loader Resolution Works

`resolve_server_jar(loader, game_version, data_dir)` is called during instance creation
and during version upgrades. It downloads or installs the server JAR into `data_dir` and
returns the path to use at startup.

The startup command is built separately in the process launch code. Each loader has its
own launch style — **the resolver must return enough info for the launcher to build the
correct command.** Return either the path to the JAR (for `-jar` style) or the path to
the generated `run.sh` / args file (for `@args.txt` style).

---

## Vanilla — Already Works

Downloads from Mojang's version manifest.
Launch: `java {jvm_args} -jar server.jar nogui`
No changes needed.

---

## Fabric — Already Works

Downloads the Fabric installer JAR, runs it via `java -jar fabric-installer.jar server`.
Launch: `java {jvm_args} -jar fabric-server-launch.jar nogui`
No changes needed.

---

## L1 — Quilt

**Installer URL:**
`https://maven.quiltmc.org/repository/release/org/quiltmc/quilt-installer/{version}/quilt-installer-{version}.jar`
Fetch the latest installer version from QuiltMC Maven metadata first.

**Install command:**
```
java -jar quilt-installer.jar install server {mc_version} --install-dir {data_dir}
```

**Launch JAR:** `quilt-server-launch.jar` — NOT `server.jar`.
The installer generates this file. Return its path from the resolver.

**Don't:** Assume the launch JAR is `server.jar` like Vanilla. It isn't.

---

## L2 — Paper

**No installer step.** The downloaded JAR is the server.

**Step 1 — Find latest build:**
```
GET https://api.papermc.io/v2/projects/paper/versions/{mc_version}/builds
```
Response includes a `builds` array. Pick the last entry with `channel = "default"` as the
latest stable build.

**Step 2 — Download JAR:**
```
GET https://api.papermc.io/v2/projects/paper/versions/{mc_version}/builds/{build}/downloads/paper-{mc_version}-{build}.jar
```

**Launch:** `java {jvm_args} -jar paper-{mc_version}-{build}.jar nogui`

**Don't:** Pick a build with `channel = "experimental"` unless no stable build exists.

---

## L3 — Forge

Only supports Minecraft 1.1–1.20.x. For 1.21+, point users to NeoForge.

**Step 1 — Find Forge version:**
```
GET https://files.minecraftforge.net/net/minecraftforge/forge/promotions_slim.json
```
Look for key `"{mc_version}-recommended"`. If not found, fall back to `"{mc_version}-latest"`.
If neither key exists, the MC version is not supported by Forge — return an error.

**Step 2 — Download installer:**
```
https://maven.minecraftforge.net/net/minecraftforge/forge/{mc_version}-{forge_version}/forge-{mc_version}-{forge_version}-installer.jar
```

**Step 3 — Run installer:**
```
java -jar forge-installer.jar --installServer {data_dir}
```

**Launch style depends on Forge version:**
- Forge **1.17+**: The installer generates `run.sh` / `run.bat` and a `libraries/` folder.
  Launch using `@user_jvm_args.txt @libraries/net/minecraftforge/.../unix_args.txt nogui`.
  Read the generated `run.sh` to get the exact args file path.
- Forge **<1.17**: Plain `java {jvm_args} -jar forge-{mc_version}-{forge_version}.jar nogui`.

**Don't:** Hardcode the args file path. Read it from the generated script.

**Don't:** Use `.output()` to run the installer. Use `.spawn()` and stream stdout/stderr
to the instance log. The installer takes 30–90 seconds and looks frozen if output is buffered.

---

## L4 — NeoForge

Only supports Minecraft **1.20.1 and above**. Return a clear error for anything older.

**Step 1 — Find latest NeoForge version:**
```
GET https://maven.neoforged.net/releases/net/neoforged/neoforge/maven-metadata.xml
```
Parse the `<release>` tag. This is the version string to use (e.g., `21.4.167`).

**Step 2 — Download installer:**
```
https://maven.neoforged.net/releases/net/neoforged/neoforge/{neoforge_version}/neoforge-{neoforge_version}-installer.jar
```

**Step 3 — Run installer:**
```
java -jar neoforge-installer.jar --installServer {data_dir}
```

**Launch:** Same `@args.txt` pattern as modern Forge (1.17+ style).
The installer generates the necessary scripts.

**Don't:** Try NeoForge on MC versions below 1.20.1. Reject them with:
`"NeoForge requires Minecraft 1.20.1 or newer. Use Forge for older versions."`

---

## Loader Launch Style Summary

| Loader    | Install step | Launch style                         |
|-----------|-------------|--------------------------------------|
| Vanilla   | No          | `java -jar server.jar nogui`         |
| Fabric    | Installer   | `java -jar fabric-server-launch.jar nogui` |
| Quilt     | Installer   | `java -jar quilt-server-launch.jar nogui` |
| Paper     | No          | `java -jar paper-{ver}-{build}.jar nogui` |
| Forge 1.17+ | Installer | `java @libraries/.../args.txt nogui` |
| Forge <1.17 | Installer | `java -jar forge-{ver}.jar nogui`   |
| NeoForge  | Installer   | `java @libraries/.../args.txt nogui` |

---

## Cross-Loader Gotcha: Streaming Installer Output

For every loader that runs an installer (Fabric, Quilt, Forge, NeoForge):
- Use `tokio::process::Command::spawn()` — not `.output()` or `.status()`
- Pipe stdout and stderr to the instance's live log stream
- The instance should be in a `"installing"` status during this phase
- Only transition to `"stopped"` (ready to start) after the installer exits with code 0
- If the installer exits with a non-zero code, set status to `"error"` and save the
  installer output as the last log entry
