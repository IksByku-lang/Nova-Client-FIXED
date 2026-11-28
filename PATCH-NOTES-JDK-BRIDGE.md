# Nova Client — Java Bridge JDK selection fix

## Root cause found in the uploaded source
`crates/horizon-jvm/src/lib.rs`, in `find_java_home()`, used
`horizon_prepare::java::find_runtime(...)`. That finder accepts a Java runtime without
`javac`, but the release Java Bridge compiles its bundled bridge/platform Java sources
at first start (`crates/horizon-jvm/src/platform.rs`). This can make Fabric startup fail
when the launcher-selected Minecraft runtime is runtime-only, even though Java itself starts.

## Patch
Changed `find_runtime` to `find`, which selects a compatible JDK with both `java` and
`javac`. Updated the function documentation to describe the actual compiler requirement.

## Important
This ZIP is a source patch, not a precompiled Windows client. This environment does not
have Cargo/Rust or a Windows runtime, so the project could not be compiled or tested here.
The patch addresses the confirmed JDK-selection bug; if the next run fails for a different
reason, its error log will identify that separate issue. It does not change Microsoft login
or the target Minecraft version (26.3).
