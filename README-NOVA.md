# Nova Client (work in progress)

Nova Client is a fork of Horizon plus its experimental Java bridge. The launcher is a GPUI app in `launcher/`.

## Without GitHub: one click on a Windows PC
Double-click `MAKE-NOVA-INSTALLER.bat` (it installs Rust the first time, and needs Python 3.12+, JDK 25 and the Microsoft C++ Build Tools). When it ends, the `dist` folder holds `horizon-launcher-setup-NovaClient.exe` (the installer) and `NovaClient-GameBuild` (optional: the game with the Java Bridge and the Nova HUD, plus `USE-NOVA-GAME-BUILD.bat`). Copy `dist` to any other PC and run the installer there: it needs no Rust. The script builds the launcher so that it never replaces itself with the official Horizon launcher. Nothing was compiled where these files were written, so the first run may show compile errors in `build-output.log`.

## Status of the build
The GitHub Actions build found compile errors in the first version; the ones that were visible are fixed (see `NOVA-CLIENT-FIX-REPORT.md`) but this version has **not** been compiled by the person who wrote the fixes. Run the build again; the log lists any that remain.

## Get the installer without building anything on your PC (GitHub)
GitHub builds it for you (the workflow `.github/workflows/nova-installer.yml`):
1. Put this project in a GitHub repository (a private one is fine) and open its **Actions** tab.
2. Choose **Nova Client - Windows installer**, press **Run workflow**, wait (the first build takes a long time).
3. Open the finished run and download the artifact **NovaClient-Installer**. Inside:
   - `horizon-launcher-setup-NovaClient.exe`: double-click it to install Nova Client for your account (no administrator), like Horizon's own setup. It is the launcher under a name the launcher recognises as the installer; it will not replace itself with the official Horizon launcher.
   - `NovaClient-GameBuild.zip` (optional): the game build with the Java Bridge and the **Nova in-game HUD**. After installing, extract it and run `USE-NOVA-GAME-BUILD.bat` to point the installed launcher at it. Without it, the launcher and all its pages work with the official game, but the Nova HUD is not in that game.
4. To publish a release with these files attached, push a tag named like `nova-v0.1.0`.
Nothing here has been run yet. If the build fails (a compile error is likely on the first run), the log shows the exact error: send it to me and I will fix it. GitHub's free minutes are limited and Windows runners count double.

## Build on your own PC (Windows)
1. `INSTALL-CARGO.bat` installs Rust via rustup.
2. `BUILD-NOVA-CLIENT.bat` builds the game and launcher; output goes to `build-output.log`.
3. `RUN-NOVA-CLIENT.bat` starts the built launcher (Cargo is not needed to run it).

Requirements: Python 3.12+, JDK 25 (Java bridge), MSVC C++ Build Tools.

## Status
**Nothing here has been compiled or tested**: the environment it was edited in had no Rust toolchain and no network. Expect a few compile errors on the first `cargo check`; the new unit tests have not been run either. Run `cargo test -p horizon-launcher` and `cargo test -p horizon-client nova_hud` after fixing them.

### Added
- **Profiles page** (`launcher/src/nova_pages.rs`): profiles are the launcher's instances (own game folder, mods, version, memory). Use, Edit (rename, icon, group), Duplicate, Mods folder, New profile, Delete with confirmation (folder goes to the Trash). An active-profile card shows the version and the Java Bridge state with mod counts.
- **Performance page**: Graphics API selector (the existing, tested one: Auto, Vulkan, DirectX 12 on Windows, OpenGL) with a restart note; Low/Balanced/High/Custom memory presets for Minecraft: Java Edition profiles only; the Nova in-game HUD switches. FPS limit, VSync, distances and fullscreen are listed as "Not available" because the launcher does not write them.
- **About page**: version, Horizon and Java Bridge credits and repository links, licence notes, logs.
- **Java mods manager** (`java_mods.rs`, Java mods tab): Enable/Disable (renames `x.jar` to `x.jar.disabled`, which the game's folder scan ignores), status per jar (Compatible, Warning, Incompatible, Missing dependency, Disabled) with the exact reasons: missing dependency ids, other Minecraft version, duplicate mod, `breaks`/`conflicts` entries. A jar is only called Compatible when its `depends.minecraft` was read and holds; otherwise it is a Warning saying so. Fabric API is reported missing when no jar in `mods-java/` provides it.
- **Crash analyzer** (`crash_analyzer.rs`): rules are now line-based and specific (no more matching every `java.lang.` line). New categories: Java not found, wrong Java version, JVM crash, Java Bridge failure, missing game files. Findings are marked "confirmed by the log" or "possible cause, not confirmed", carry a confidence, the matching log lines and suggested fixes; with no match it says the cause could not be identified. The crash card on Home shows all of it.
- **In-game Nova HUD**: `crates/horizon-client/src/nova_hud.rs` (config and lines) and `game/nova_hud.rs` (drawn after the F3 overlay, only in the world, not with F3 open or F1). Shows FPS, frame time, memory (the F3 memory line), coordinates and the profile name, in the top-left or top-right corner, small/normal/large. The launcher saves the choices in `nova-overlay.txt` and passes them as `NOVA_OVERLAY` / `NOVA_PROFILE` when it starts the game.
- Windows scripts: `INSTALL-CARGO.bat`, `BUILD-NOVA-CLIENT.bat` (writes `build-output.log`), `RUN-NOVA-CLIENT.bat`.

### Concept-image pass (latest)
- **Sidebar**: Nova logo and wordmark over a navigation list (Home, Mods, Profiles, Performance, Settings, About) with icons; the old top-bar tabs for those pages are gone (Servers, Wardrobe, Shop and Versions stay there). The instance list below is now headed "Profiles". New glass icon `nova.svg` (`crates/horizon-theme`).
- **Home**: three status tiles under Play (selected profile, Java Bridge on/off for that profile, game version) and a Quick Actions row (Manage Mods, Create Profile, Settings, View Logs). All read real state.
- **HUD**: F10 hides or shows it in the game while it is switched on in the launcher.
- **Settings**: "Reset settings" with a confirmation, in the About card; it keeps the selected version and profile, and profiles, mods, worlds and accounts are untouched. Restart the launcher afterwards.
- **About**: the Nova mark is drawn from the icon.

### Latest pass (from NOVA-CLIENT TODO)
- **Crash Analyzer page** (sidebar, `nova_crashes.rs` + `nova_crash_history.rs`): Analyze Latest Log, Open Crash Reports Folder, a saved history with search, certainty filter (All / Confirmed / Possible), category filter and Newest/Oldest order, a details view (date and time, log file, Minecraft version and profile at the time, category, certainty and confidence, matching log lines, mods named in the log, numbered steps), Copy Report, Export TXT, Export JSON, Open Original Log, delete one entry, and Clear History with confirmation. Crashes the launcher sees are analysed and saved on their own. The history is `crash-history.jsonl` in `%LOCALAPPDATA%\NovaClient`; exports go to `NovaClient\exports`. A finding is labelled "Confirmed by the log", "Possible cause, not confirmed" or "No cause could be identified"; nothing is shown as a guarantee.
- **Play warning**: a profile with Fabric mods on and a serious mod problem (an incompatible jar or a missing dependency) warns once with a message; pressing Play again starts anyway.
- **Mod list export** (Home Mods card): writes `<profile>-mods.txt` and `.json` to `NovaClient\exports`.
- **Profile export and import** (Profiles page): Export writes `NovaClient\profiles\<name>.nova-profile.json` with settings only (name, group, Fabric mods on or off, and for Java Edition profiles the version, loader, memory, Java arguments; never the Java path, mods or worlds). Import Profile asks for such a file, checks it and creates a new profile that never takes an existing name.

### Mods, video options, activity and icon (this pass)
- **Java mods list**: search (name, file, id, author), filter (All / Enabled / Disabled / Problems), sort (Name / Status), the author names, version, loader and Minecraft version as the jar states them (a jar that does not state a version reads "Minecraft version not stated"), Export list (TXT and JSON). Enable/Disable, Remove with confirmation, Refresh, Open folder and Add JAR were already there.
- **Performance > Video options** now really write the active profile's `options.txt` (keys the game reads: `maxFps` 10-260 where 260 is unlimited, `enableVsync`, `fullscreen`, `renderDistance`, `simulationDistance`), with a Low/Balanced/High distance preset. Values outside what the game accepts are refused; every other line of the file is kept; the original is copied once to `options.txt.nova-backup`. Refused while the game runs (it rewrites the file when it closes) and when the profile has no `options.txt` yet (start the game once).
- **Home > Recent Activity**: the last five events from profiles' last play times and the crash history.
- **Application icon**: `packaging/icons/nova.ico` (used by `launcher/build.rs`, which falls back to `horizon.ico`).

### Mods list and branding, finished (latest pass)
- **Java mods list**: each jar shows its own icon (a puzzle piece when it has none), name, version, author, loader and Minecraft version. **Loader check**: a Forge, NeoForge or Quilt jar is marked Incompatible ("This is a Forge mod. The Java Bridge runs Fabric mods only."); a jar with no mod metadata is a Warning. **Check updates** asks Modrinth, by each jar's SHA-1, for the newest version for Fabric and the Java Bridge's Minecraft version; only a different file counts as an update, mods Modrinth does not know are skipped (not called up to date). **Update** downloads the new jar (host allow-list, size limit and checksum checked), then swaps it for the old one; refused while the game has the old jar open. **Import modpack** takes a Modrinth `.mrpack`: refused before any download when it is for another Minecraft version or another loader; otherwise it installs only the pack's client-side `mods/*.jar` (and `overrides/mods` jars) into `mods-java/`, skipping jars already there; resource packs, configs and worlds in the pack are ignored.
- **Branding**: Apps & features shows "Nova Client" (publisher "Nova Client (built on Horizon)"), the installer screens say Nova Client, the exe's Properties show product name "Nova Client" and keep the Horizon (MIT) notice, the Cargo description says Nova Client. **Not renamed on purpose**: the install folder "Horizon Launcher", its shortcuts, the registry keys, `horizon-launcher.exe`, the updater and every engine identifier, because renaming them would stop existing installs from being found, updated or removed.
- Still named Horizon in the UI where it names the engine: the Horizon tile in Settings, the Horizon edition tab, and Horizon instance icons.

### Account block and new artwork (latest)
- **Sidebar account block** (under the profile list): when a Microsoft account is selected, its head, name and "Microsoft account"; a click opens Settings to manage accounts. When nobody is signed in it is a **Sign in with Microsoft** button that starts the launcher's real sign-in (the same one as the top bar's). Under it, the Nova slogan card: "Nova Client / Play. Optimize. Enjoy."
- **Home artwork** replaced: `launcher/assets/nova-hero.png` is now a pixel-style sunset scene (a Nova crystal tower on floating islands over a lake) drawn by a script, **not an AI-generated image** (no image generator was available). It is 856x1004 and shown scaled down on the right of Home's head. Replace the file with any PNG of a similar shape to change it; no code change needed.

### Renderer, RAM and artwork
- **Performance > Renderer**: radio list Auto / Vulkan / OpenGL (+ DirectX 12 on Windows). Choosing one writes the game's graphics file (`backend::set_graphics_api`), which the game reads at its next start; the page says so, and says to close the game first if it is running.
- **Performance > Active profile**: the launcher's existing memory slider (drag, or type MB), saved to the profile as you set it and used as `-Xmx` at the next Play, plus the Low/Balanced/High presets. Only for Minecraft: Java Edition profiles: Horizon's native game has no heap to size, and the page says that when the active profile is a Horizon one.
- **Home artwork**: the castle-and-player art cropped from the concept image (`launcher/assets/nova-hero.png`; the full concept sheet is `launcher/assets/nova-client-concept.png`) is drawn faded at the right of Home's head.

### Home dashboard (from the second concept image)
Under Play, Home now shows four tiles (profile, Java Bridge, game version, mods installed) and six cards, all reading real state:
- **Mods**: the active profile's first four Java mods with their status (Compatible, Warning, Incompatible, Missing dependency, Disabled) and a working on/off switch; View All opens the Mods page. If Fabric mods are off for the profile, it says so.
- **Profiles**: up to three profiles as cards with Select; Create Profile opens the existing dialog.
- **Performance**: the Graphics API choice (written to the file the game reads at next start) and the RAM slider for Java Edition profiles.
- **In-Game Overlay**: whether the HUD is on and which elements; Configure Overlay opens Performance.
- **Crash Analyzer**: "Open Analyzer" reads the newest log or crash report (last 512 KiB, `.gz` skipped), shows the category, confirmed or possible, confidence, the matching lines and two fixes, with Copy report and Open log. It says so when there is no log or no cause found.
- **Quick Actions**: Open Mods Folder, Open Logs Folder, Check for Updates (the result appears in Settings).

### Mod search on Home (Modrinth)
The Mods card has a search box. It searches **Modrinth** (in the background, filtered to the profile's Minecraft version and loader) and lists the first five results with an **Add** button.
- **Java Edition profile**: Add uses the launcher's existing mod browser installer (`browser::java_instance::install`): required dependencies are installed too, every download is checked against Modrinth's published checksum, and the mod lands in the profile's `mods/` folder.
- **Horizon profile with Fabric mods on**: Fabric mods for the Java Bridge's Minecraft version (`java_mods::MINECRAFT`) are downloaded, with their required dependencies and checksum checks, into `mods-java/`. The Mods page then shows them with their status. Restart the game to load them.
- A Horizon profile with Fabric mods off cannot take mods: the card says so.
- **CurseForge is not offered.** Its API needs a developer key, which the launcher does not ship. The existing source (`browser/curseforge.rs`) only turns on with `HORIZON_CURSEFORGE_API_KEY`, and the launcher lists only Modrinth and Horizon's registry. The card says so instead of showing a dead tab.
- A mod listed by Modrinth is not proof that it works in the Java Bridge: check its status on the Mods page after adding it.

### Not done from the TODO (be aware)
- Visible Horizon branding is only partly replaced (sidebar, About, icon, window title). The Settings engine tile, the Horizon edition tab and instance icons keep Horizon's name because they name the engine.
- Mods: the Horizon Mods page is unchanged. Updates for mods Modrinth does not know (CurseForge-only or hand-made jars) cannot be checked.
- Add JAR still copies a jar made for another Minecraft version and then flags it, rather than refusing it.
- Home profile and version selectors are Horizon's own existing controls; they were not re-audited.
- Installer branding is limited to the visible names (see above); the release workflow files and the website were not touched.
- Conflict detection uses fabric.mod.json `breaks`/`conflicts` only, not crash logs.
- No custom application icon or installer branding.
- None of this has been compiled or run.

### Not done / limitations
- Not a pixel copy of the concept: no account chip or tagline card in the sidebar, no VSync, Fullscreen or Apply Settings in the Performance card (the launcher cannot write those options; the other settings save as you change them), no full-width hero artwork behind Home, no account chip in the sidebar, no per-profile cards grid, no "Run Performance Test" button (it would need a real benchmark). The launcher keeps its existing purple glass look; the new pages are made for the dark look.
- Performance lists OpenGL as available because the renderer really has it; the concept's "Unavailable" label would be false.
- F10 may clash if a player binds F10 to a game action.
- Home's Play button and Java mods chip are Horizon's own; they were not re-audited.
- HUD: no ping (no reading exists in the client), no bottom corners or custom colours, memory comes from the F3 memory line.
- Memory presets and JVM arguments apply to Minecraft: Java Edition profiles only; Horizon's native game has no heap to size. Custom JVM arguments are edited in the profile's Edit dialog, not on Performance.
- Mod dependency checks read `fabric.mod.json` only; a dependency bundled as a nested jar inside another mod is not recognised and would be reported missing. The Java Bridge does not support every Fabric API: see `docs/java-bridge.md`.
- Settings keeps Horizon's existing page (theme, language, game folder, Java path, updates, logs); there is no separate reset-all button added.
- Horizon and Java Bridge attribution and licences are unchanged (`LICENSE`, `THIRD_PARTY_NOTICES.md`).
