# Nova Client — implementation notes

This is a development fork of the supplied Horizon + Java Bridge source archive.
The image at `launcher/assets/nova-client-concept.png` is the approved visual direction
(reference artwork); it is not a screenshot of the running launcher and is not used as
a fake full-window UI background.

## What is present in the source already

- **Home / Play:** the existing launcher home and instance selection.
- **Mods:** the existing mod browser and installed-mod management, including the
  Java-mods bridge page where enabled by the build.
- **Profiles:** Horizon launcher instances already isolate game versions, loaders,
  mods and settings. This is the foundation for Nova profiles; a separate profile
  dashboard matching the concept is not yet implemented.
- **Performance / Graphics API:** the existing Settings > Game selector persists
  the renderer choice for the next game start. Supported choices are platform
  dependent: Windows Auto/Vulkan/DirectX 12/OpenGL, Linux Auto/Vulkan/OpenGL,
  macOS Auto (Metal backend).
- **Crash Analyzer:** `launcher/src/crash_analyzer.rs` contains offline heuristic
  analysis for common JVM, mod, Mixin, memory, graphics, network and native failures.
  The existing crash notice uses this analysis; a dedicated searchable history page
  and export bundle UI are not yet implemented.
- **In-game overlays:** the Horizon client already has native overlay/HUD code and
  the Java bridge includes overlay integration points. A Nova-branded configurable
  overlay matching the concept is not yet implemented.

## Nova branding applied in this pass

The launcher window title, custom titlebar label, macOS app menu and application ID
now use Nova Client. The concept artwork is included in `launcher/assets/` for the
next UI implementation pass. Internal Horizon engine/package identifiers remain
unchanged where changing them would break the build, update paths or bridge integration.

## Important limitations

This is still a development source archive, not a ready-to-run installer. The full
concept layout (left navigation, redesigned Home/Mods/Profiles/Performance pages),
custom Nova logo artwork, dedicated crash-history page, and Nova overlay controls
still need implementation. No successful Rust build or runtime test has been run
in this environment. The Java Bridge remains experimental and does not guarantee
compatibility with every Fabric mod.
