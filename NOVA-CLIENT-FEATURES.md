# Nova Client — feature integration status

This build is a Nova-branded development fork of the supplied Horizon source tree.
The upstream engine and internal Horizon identifiers are intentionally retained where
changing them would break release discovery, file formats, protocol diagnostics, or
engine compatibility. The visible launcher title bar and app identity are branded Nova.

## Included from the supplied source

### 1. Nova launcher identity
- Visible title bar and operating-system app identity use **Nova Client**.
- Existing launcher layout, account handling, game installation and update flows remain.
- This is a first branding pass, not a complete replacement of upstream artwork,
  installer metadata, release endpoints, or in-game engine branding.

### 2. Mod Manager
- Existing Mods page supports Horizon-native mods and Java mod projects from Modrinth.
- It includes project search, install/update/remove flows, Java mod inspection,
  modpack import, and native mod status checks.
- The Modrinth/Java mod pipeline is not a guarantee that every Fabric mod works.
  The Java Bridge remains opt-in/beta and must be tested with each target game build.
- A full automatic conflict graph across all Fabric mods is not claimed here.

### 4. Performance and Graphics API settings
- Existing Settings > Game contains graphics API selection and stores the selected
  value for the next launch.
- Supported options are constrained by the current engine/platform implementation;
  the launcher must not advertise a backend that the engine cannot initialize.
- Existing game settings and native build profile controls remain available.

### 5. Profiles
- Existing instance management provides separate game folders/configurations, versions,
  loaders, mod sets and launch targets. Treat each instance as a profile.
- This update does not yet add a separate profile export/import wizard or one-click
  cloning UI.

## Not implemented yet: 6. Nova In-Game Overlay

A real overlay showing FPS, frame time, ping and memory requires a HUD integration in
the running game renderer and reliable runtime telemetry. The launcher cannot display
 those values over the game merely by adding a settings toggle. This feature is therefore
 left unadvertised and is not represented as complete in this build.

## Build status

This is source code, not a tested Windows executable. Build with the included
`BUILD-NOVA-CLIENT.bat` after installing the documented prerequisites. A successful
Windows build and in-game validation are still required before calling this release stable.
