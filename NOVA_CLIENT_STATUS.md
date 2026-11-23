# Nova Client: current integration

This source tree is based on the supplied Horizon `java-bridge-updated` branch archive.
It already contains the Horizon Rust client, launcher, `horizon-jvm` crate and the
opt-in `java-mods` feature. The Java bridge is not reimplemented here.

## Nova crash diagnosis

The launcher now includes an offline crash-analysis module at
`launcher/src/crash_analyzer.rs`. The Home crash notice uses it to show a likely
cause, a heuristic confidence score, remediation suggestions and possible mod IDs.

Supported signatures include:

- Out of memory / allocation failures
- Vulkan device loss and initialization errors
- Graphics-driver failures
- Mixin application failures
- Missing mod dependencies / mod-resolution errors
- Java and JVM startup errors
- Fabric mod loading or execution errors
- Network connection failures
- Native access violations
- Horizon/Rust panics

The analyzer is heuristic. It must not be presented as a definitive diagnosis.
It does not upload logs. Existing Horizon crash-report privacy and sharing controls
remain unchanged.

## Graphics API selection

The launcher Settings > Game section now exposes a Graphics API selector. It writes
`graphicsApi` to `horizon-graphics.txt` and applies the selection on the next game
start. Supported choices are Auto, Vulkan and OpenGL on Linux; Auto, Vulkan,
DirectX 12 and OpenGL on Windows. macOS remains on its platform backend (Metal).
The in-game Video Settings > Graphics API control remains available as well.
The launcher preserves all other graphics settings when changing this value.

## Java bridge

The supplied branch's documented build path is:

```sh
cargo build --profile fast -p horizon-client --features horizon-client/java-mods
cd launcher
cargo build --release
```

The Java bridge is beta and opt-in. Its documented requirements and limitations
are in `docs/java-bridge-beta.md`. In particular, this is not a promise that every
Fabric mod, renderer replacement, or Minecraft version works.

## Build prerequisites

Follow the upstream Horizon build documentation and the Java bridge beta guide.
The supplied repository README specifies Rust 1.95+ and Python 3.11+; the Java
bridge beta guide documents JDK 25. A successful build has not been verified in
this environment because the Rust toolchain is not installed here.

## Remaining Nova work

- Replace Horizon product identity with Nova Client consistently across launcher,
  game window, installer, release metadata, update endpoints and documentation.
- Add a dedicated Crash Analyzer page with searchable history and exportable
  diagnostic bundles.
- Improve mod conflict/dependency analysis using the bridge's actual loader output.
- Add Nova-specific profile management and compatibility UI.
- Build and run the Windows client, then test Fabric 26.3 mods and crash scenarios.

Do not distribute this development archive as a finished executable.
