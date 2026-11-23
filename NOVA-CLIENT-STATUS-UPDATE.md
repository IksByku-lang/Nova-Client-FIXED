# Nova Client — work update

This update was made against the supplied `NovaClient-Final.zip`.

## Changes in this pass

- Crash Analyzer now appends a local JSON Lines history entry after each analysis.
- History is stored under the user's local application data directory: `NovaClient/crash-history.jsonl`.
- Each entry contains timestamp, source filename, category, summary, confidence and whether the finding is explicitly confirmed by the log. Raw logs are not copied.
- The Home Crash Analyzer card now has **Analyze Latest** and **Open History** actions.
- Existing profile create/edit/duplicate/delete/select, per-profile mods and memory settings, graphics API choices and HUD configuration were preserved.

## Still incomplete / needs validation

- Dedicated in-launcher searchable crash history and diagnostic bundle export.
- Full profile archive import/export (existing profile duplication is supported).
- Full mod conflict graph and loader-derived compatibility details.
- FPS cap, VSync, render/simulation distance and fullscreen controls are not wired to engine settings in this branch; they remain clearly marked unavailable.
- Ping is not available to the current HUD telemetry.
- Full replacement of remaining Horizon branding in engine/installer/release metadata.
- Java Bridge compatibility work: this is beta and requires real mod/version testing.
- Full build and Windows runtime validation.

## Build status

Rust/Cargo is not installed in the editing environment, so this source update could not be compiled here. The ZIP is checked for archive integrity, not represented as a verified executable. Run `BUILD-NOVA-CLIENT.bat` on Windows with the prerequisites from the project README, then fix any compiler errors before distributing.
