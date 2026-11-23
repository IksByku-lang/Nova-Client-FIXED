# Nova Client — repair notes

Date: 2026-11-22
Source archive: `NovaClient-Final.zip`

## Changes

- Fixed the launcher compile error in `launcher/src/java_modpack.rs`. With `zip = 9.1.3`, `ZipArchive::file_names()` yields fallible entry names (`Result<Cow<str>, ZipError>`). The code now converts each name to an owned `String`, collects ZIP errors, and returns a readable error instead of trying to pass `Result` to `str::to_string`.
- Updated `.github/workflows/nova-windows-build.yml` to compile the launcher with `HORIZON_LAUNCHER_NO_SELF_UPDATE=1`, so the Nova fork does not self-replace with the upstream Horizon launcher.
- Improved `BUILD-NOVA-CLIENT.bat`: checks Python 3.12+, checks that the full JDK (including `javac`) is present, verifies Java 25, and sets the no-self-update build flag.
- Updated `RUN-NOVA-CLIENT.bat` to set the no-self-update runtime flag too.

## Checks completed

- Original ZIP passed its integrity check (7,844 entries).
- Cargo manifests and both lockfiles parsed; all declared local Cargo path dependencies and workspace members were found.
- YAML workflows and regular JSON files parsed successfully. Two TOML files under `crates/cargo-horizon-mod/template/` are templates rather than valid manifests; `crates/horizon-perf/tests/golden/follow.json` is newline-delimited JSON data, not a single JSON document.
- Python sources under `launcher`, `tools`, `benchmarks`, and `tests` passed `compileall`.
- The Java-mod release feature tests passed (2 tests). The isolated build-times regression test that was reported as an error during the interrupted full suite also passed when run on its own.

## Not verified here

- A Rust/Cargo build or Rust unit tests could not be run: this editing environment has no `rustc` or `cargo`, and network DNS is unavailable to install them.
- The complete Python unittest suite did not finish within the available execution window.
- The Windows installer and the actual game launch have not been tested on a Windows machine.

The ZIP therefore contains source fixes and static checks, not a pre-verified Windows executable. Run the GitHub Actions Windows build after pushing these files; its result is the definitive compile check.


## Second repair pass (from the GitHub Actions build errors)

- `E0433` `Field`: `nova_pages.rs`, `nova_crashes.rs` and `java_mods_page.rs` used `servers_page::Field` without importing it. Added `use crate::servers_page::Field;` to all three.
- `E0616` private fields: the fields of `NovaUi` (`mods`, `install_note`, ...) are now `pub(super)`, so `java_mods_page.rs` can use them.
- `E0599` `collect` in `java_modpack.rs`: with this project's `zip`, `file_names()` yields `Result` names (the existing `datapacks.rs` reads it the same way); the names are now collected with the error reported.
- Reset settings no longer clears the game and server folders (it would have made every profile appear to vanish).
- The crash analyzer's mod-id search no longer slices a line with offsets taken from a Unicode-lowercased copy (could panic on some non-ASCII logs).
- Other errors in the same build were not visible to the person who fixed this; none of this has been compiled here, so run the Windows build again and send the remaining errors.
