//! Nova Client crash diagnosis for Horizon and Fabric Java-mod logs.
//!
//! This module is deliberately offline: it analyses text already present on the player's
//! computer and never uploads logs. Each rule looks for signatures on a single log line. A rule
//! whose signature is an explicit error message gives a *confirmed* finding; a rule that only
//! infers a cause gives a *possible* one, with a lower confidence. Nothing is reported when no
//! rule matches.
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CrashKind {
    MissingJava,
    WrongJavaVersion,
    OutOfMemory,
    JvmCrash,
    GraphicsDriver,
    Vulkan,
    JavaRuntime,
    FabricMod,
    Mixin,
    MissingDependency,
    BridgeInit,
    MissingGameFiles,
    Network,
    NativeAccessViolation,
    GamePanic,
    Unknown,
}

impl CrashKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::MissingJava => "Java runtime not found",
            Self::WrongJavaVersion => "Incorrect Java version",
            Self::OutOfMemory => "Out of memory",
            Self::JvmCrash => "Java virtual machine crash",
            Self::GraphicsDriver => "Graphics driver failure",
            Self::Vulkan => "Vulkan initialization or device failure",
            Self::JavaRuntime => "Java runtime failure",
            Self::FabricMod => "Fabric mod failure",
            Self::Mixin => "Mixin application failure",
            Self::MissingDependency => "Missing mod dependency",
            Self::BridgeInit => "Java Bridge failure",
            Self::MissingGameFiles => "Missing game files",
            Self::Network => "Network or connection failure",
            Self::NativeAccessViolation => "Native access violation",
            Self::GamePanic => "Game panic",
            Self::Unknown => "Unknown failure",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Severity {
    Info,
    Warning,
    Error,
    Critical,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CrashAnalysis {
    pub kind: CrashKind,
    pub severity: Severity,
    /// 0..=100; heuristic confidence, not a probability.
    pub confidence: u8,
    /// True when the log holds an explicit error message for this cause; false when the cause
    /// is only inferred, or when nothing matched.
    pub confirmed: bool,
    pub summary: String,
    pub probable_mods: Vec<String>,
    /// What was found, in words.
    pub evidence: Vec<String>,
    /// The log lines that matched (at most three), trimmed.
    pub excerpt: Vec<String>,
    pub suggestions: Vec<String>,
}

/// A line matches a pattern when it contains every substring of it (compared in lower case).
type Pattern = &'static [&'static str];

struct Rule {
    kind: CrashKind,
    confirmed: bool,
    confidence: u8,
    patterns: &'static [Pattern],
    summary: &'static str,
    evidence: &'static str,
    suggestions: &'static [&'static str],
}

/// In order: the first rule with a match decides.
const RULES: &[Rule] = &[
    Rule {
        kind: CrashKind::MissingJava,
        confirmed: true,
        confidence: 95,
        patterns: &[
            &["'java' is not recognized"],
            &["java: command not found"],
            &["no java runtime present"],
            &["unable to locate a java"],
            &["cannot find java"],
            &["could not find java"],
        ],
        summary: "No Java runtime could be started.",
        evidence: "The log says Java could not be found.",
        suggestions: &[
            "Install a JDK (Java 25 for the Java Bridge beta) and select it in Settings.",
            "If Java is installed, set its folder as the Java runtime path.",
        ],
    },
    Rule {
        kind: CrashKind::WrongJavaVersion,
        confirmed: true,
        confidence: 95,
        patterns: &[
            &["unsupportedclassversionerror"],
            &["class file version"],
            &["requires java"],
        ],
        summary: "The game or a mod needs a newer Java than the one that ran it.",
        evidence: "The log reports a class-file or Java version mismatch.",
        suggestions: &[
            "Select a newer JDK in Settings (the Java Bridge beta documents Java 25).",
            "Check that a mod was not built for a newer Java than the runtime.",
        ],
    },
    Rule {
        kind: CrashKind::OutOfMemory,
        confirmed: true,
        confidence: 96,
        patterns: &[
            &["java.lang.outofmemoryerror"],
            &["gc overhead limit exceeded"],
            &["failed to allocate memory"],
            &["out of memory"],
        ],
        summary: "The process ran out of available memory.",
        evidence: "The log contains an out-of-memory signature.",
        suggestions: &[
            "Close memory-heavy applications before launching.",
            "Increase the profile's memory only if the computer has enough free RAM.",
            "With a large mod set, test with half the mods to find heavy content.",
        ],
    },
    Rule {
        kind: CrashKind::JvmCrash,
        confirmed: true,
        confidence: 93,
        patterns: &[
            &["a fatal error has been detected by the java runtime"],
            &["hs_err_pid"],
        ],
        summary: "The Java virtual machine itself crashed.",
        evidence: "The log mentions a fatal JVM error or an hs_err file.",
        suggestions: &[
            "Open the hs_err_pid file beside the game for the failing frame.",
            "Update the JDK and the graphics driver, then retry without mods.",
        ],
    },
    Rule {
        kind: CrashKind::Vulkan,
        confirmed: true,
        confidence: 95,
        patterns: &[
            &["vk_error_device_lost"],
            &["vulkan device lost"],
            &["failed to create vulkan"],
            &["vk_error_initialization_failed"],
            &["vk_error_incompatible_driver"],
        ],
        summary: "Vulkan could not initialize or the graphics device was lost.",
        evidence: "A Vulkan device or initialization error was found.",
        suggestions: &[
            "Update the graphics driver from the GPU maker's website.",
            "In Performance, switch the Graphics API to OpenGL or DirectX 12 and restart the game.",
            "Remove GPU overclocking and retest.",
        ],
    },
    Rule {
        kind: CrashKind::GraphicsDriver,
        confirmed: true,
        confidence: 88,
        patterns: &[
            &["dxgi_error_device_removed"],
            &["device hung"],
            &["graphics device lost"],
            &["gpu crash"],
        ],
        summary: "The graphics device or driver stopped responding.",
        evidence: "The log contains a graphics-device failure signature.",
        suggestions: &[
            "Update or clean-install the graphics driver.",
            "Disable GPU overclocking and overlays temporarily.",
            "Try another Graphics API in Performance.",
        ],
    },
    Rule {
        kind: CrashKind::Mixin,
        confirmed: true,
        confidence: 94,
        patterns: &[
            &["mixinapplyerror"],
            &["mixin apply failed"],
            &["critical injection failure"],
            &["invalidinjectionexception"],
            &["mixintransformererror"],
        ],
        summary: "A Mixin could not be applied: a mod targets code that is not there, or two mods clash.",
        evidence: "A Mixin application or injection failure was found.",
        suggestions: &[
            "Check that every mod targets the Minecraft version the Java Bridge runs.",
            "Update the mod named in the excerpt and its dependencies.",
            "Disable recently added mods one by one to isolate the clash.",
        ],
    },
    Rule {
        kind: CrashKind::MissingDependency,
        confirmed: true,
        confidence: 92,
        patterns: &[
            &["requires any version of"],
            &["requires version", "which is missing"],
            &["which is missing"],
            &["missing dependency"],
            &["unmet dependency"],
            &["incompatible mod set"],
            &["mod resolution failed"],
        ],
        summary: "The mod loader could not find a mod that another mod needs.",
        evidence: "The log names a dependency that is missing or has the wrong version.",
        suggestions: &[
            "Install the dependency named in the excerpt.",
            "The Mods page shows which jar needs what under Missing dependency.",
            "Remove duplicate or incompatible versions of the same mod.",
        ],
    },
    Rule {
        kind: CrashKind::BridgeInit,
        confirmed: true,
        confidence: 90,
        patterns: &[
            &["[java mods]", "failed"],
            &["[java mods]", "could not"],
            &["horizon_jvm", "error"],
            &["could not create the java virtual machine"],
            &["error occurred during initialization of vm"],
        ],
        summary: "The Java Bridge or the JVM it starts failed to initialize.",
        evidence: "A Java Bridge or JVM start-up failure was logged.",
        suggestions: &[
            "Check the Java runtime path and that it is Java 25 or newer.",
            "Turn Fabric mods off for this profile to confirm the game starts without the bridge.",
            "Check the bridge's files exist next to the game, and read docs/java-bridge.md.",
        ],
    },
    Rule {
        kind: CrashKind::MissingGameFiles,
        confirmed: false,
        confidence: 65,
        patterns: &[
            &["no such file or directory", "assets"],
            &["cannot find the file", "assets"],
            &["cannot find the path", "assets"],
            &["is not installed"],
            &["failed to read", "resources"],
        ],
        summary: "Game files the game tried to read seem to be missing.",
        evidence: "A file or version the game needed could not be found.",
        suggestions: &[
            "Open Versions and install the version again.",
            "Check that antivirus software did not quarantine files in the game folder.",
        ],
    },
    Rule {
        kind: CrashKind::NativeAccessViolation,
        confirmed: true,
        confidence: 91,
        patterns: &[
            &["exception_access_violation"],
            &["0xc0000005"],
            &["segmentation fault"],
            &["sigsegv"],
        ],
        summary: "A native component or driver accessed invalid memory.",
        evidence: "A native access-violation signature was found.",
        suggestions: &[
            "Update graphics drivers and the operating system.",
            "Temporarily remove overlays and injected tools.",
            "Keep the full log for a bug report.",
        ],
    },
    Rule {
        kind: CrashKind::GamePanic,
        confirmed: true,
        confidence: 86,
        patterns: &[&["panicked at"], &["---- horizon crash report ----"]],
        summary: "The native game reported a panic.",
        evidence: "A Horizon (Rust) panic or crash report marker was found.",
        suggestions: &[
            "Read the first panic message and the earliest relevant stack frame.",
            "Update Horizon and retest without optional mods.",
            "Keep the complete crash report for a bug report.",
        ],
    },
    Rule {
        kind: CrashKind::FabricMod,
        confirmed: false,
        confidence: 60,
        patterns: &[
            &["net.fabricmc.loader", "exception"],
            &["net.fabricmc.loader", "failed"],
            &["fabric", "mod", "crash"],
        ],
        summary: "The failure seems to involve Fabric mod loading, but the log gives no exact cause.",
        evidence: "Fabric loader lines appear beside an error.",
        suggestions: &[
            "Check the Mods page for a mod marked Incompatible or Missing dependency.",
            "Disable the most recently added mod and start again.",
        ],
    },
    Rule {
        kind: CrashKind::Network,
        confirmed: true,
        confidence: 88,
        patterns: &[
            &["connection timed out"],
            &["connection refused"],
            &["unknown host"],
            &["failed to connect"],
        ],
        summary: "The log points to a network or server problem rather than a local crash.",
        evidence: "A connection or DNS failure signature was found.",
        suggestions: &[
            "Check the internet connection and the server address.",
            "Check firewall or VPN rules if it keeps failing.",
        ],
    },
    Rule {
        kind: CrashKind::JavaRuntime,
        confirmed: false,
        confidence: 55,
        patterns: &[&["jvm.dll", "error"], &["java runtime", "error"]],
        summary: "The Java runtime reported an error, but not which cause.",
        evidence: "A Java runtime error line was found without a more specific signature.",
        suggestions: &[
            "Use the Java version the Java Bridge beta documents (Java 25).",
            "Check that the Java build matches the operating system's architecture.",
        ],
    },
];

/// Fabric names the mods it complains about like `Mod 'Name' (id)`, `mod id: id` or `modid=id`.
fn mod_ids(text: &str) -> Vec<String> {
    let mut found = BTreeSet::new();
    for line in text.lines() {
        // ASCII lowercase keeps byte offsets, so `pos` is valid in `line` (full lowercase can change lengths).
        let lower = line.to_ascii_lowercase();
        for marker in ["mod id:", "modid="] {
            if let Some(pos) = lower.find(marker) {
                let tail = line[pos + marker.len()..].trim_start();
                let id: String = tail
                    .chars()
                    .take_while(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'))
                    .collect();
                if (2..=64).contains(&id.len()) {
                    found.insert(id);
                }
            }
        }
        // `Mod 'Some Name' (some_id)`: the id is the word in parentheses after the name.
        if let Some(pos) = lower.find("mod '") {
            if let Some(open) = line[pos..].find(" (") {
                let tail = &line[pos + open + 2..];
                if let Some(close) = tail.find(')') {
                    let id = &tail[..close];
                    if (2..=64).contains(&id.len())
                        && id
                            .chars()
                            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'))
                    {
                        found.insert(id.to_string());
                    }
                }
            }
        }
    }
    found.into_iter().collect()
}

/// The lines of `log` that match a pattern of `rule`, trimmed to 200 characters, at most three.
fn excerpt(log: &str, rule: &Rule) -> Vec<String> {
    log.lines()
        .filter(|line| {
            let lower = line.to_lowercase();
            rule.patterns
                .iter()
                .any(|p| p.iter().all(|needle| lower.contains(needle)))
        })
        .take(3)
        .map(|line| line.trim().chars().take(200).collect())
        .collect()
}

pub fn analyze(log: &str) -> CrashAnalysis {
    let mods = mod_ids(log);
    for rule in RULES {
        let lines = excerpt(log, rule);
        if lines.is_empty() {
            continue;
        }
        let mut evidence = vec![rule.evidence.to_string()];
        if !mods.is_empty() {
            evidence.push(format!("Mod identifiers named in the log: {}.", mods.join(", ")));
        }
        return CrashAnalysis {
            kind: rule.kind,
            severity: match rule.kind {
                CrashKind::Network => Severity::Warning,
                _ => Severity::Error,
            },
            confidence: rule.confidence,
            confirmed: rule.confirmed,
            summary: rule.summary.to_string(),
            probable_mods: mods,
            evidence,
            excerpt: lines,
            suggestions: rule.suggestions.iter().map(|s| s.to_string()).collect(),
        };
    }
    CrashAnalysis {
        kind: CrashKind::Unknown,
        severity: Severity::Warning,
        confidence: 0,
        confirmed: false,
        summary: "Nova could not identify the cause from this log.".into(),
        probable_mods: mods,
        evidence: vec!["No known crash signature matched.".into()],
        excerpt: Vec::new(),
        suggestions: vec![
            "Open the full latest.log and the crash report, if there is one.".into(),
            "Try again with optional mods disabled.".into(),
            "Share the report in a bug report: it never leaves your computer by itself.".into(),
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_out_of_memory() {
        let a = analyze("java.lang.OutOfMemoryError: Java heap space");
        assert_eq!(a.kind, CrashKind::OutOfMemory);
        assert!(a.confirmed);
        assert!(a.confidence >= 90);
        assert_eq!(a.excerpt.len(), 1);
    }

    #[test]
    fn detects_mixin_failure() {
        let a = analyze("org.spongepowered.asm.mixin.transformer.throwables.MixinApplyError");
        assert_eq!(a.kind, CrashKind::Mixin);
    }

    #[test]
    fn a_plain_stack_frame_is_not_a_mixin_failure() {
        let a = analyze("at org.spongepowered.asm.mixin.Mixins.addConfiguration(Mixins.java:1)");
        assert_eq!(a.kind, CrashKind::Unknown);
    }

    #[test]
    fn detects_vulkan_device_loss() {
        let a = analyze("VK_ERROR_DEVICE_LOST while submitting frame");
        assert_eq!(a.kind, CrashKind::Vulkan);
    }

    #[test]
    fn a_generic_java_exception_is_not_blamed_on_the_runtime() {
        let a = analyze("java.lang.NullPointerException: Cannot invoke foo()");
        assert_eq!(a.kind, CrashKind::Unknown);
    }

    #[test]
    fn detects_a_missing_fabric_dependency_and_names_the_mod() {
        let log = "Mod 'Cool Mod' (coolmod) 1.0 requires any version of fabric-api, which is missing!";
        let a = analyze(log);
        assert_eq!(a.kind, CrashKind::MissingDependency);
        assert!(a.probable_mods.contains(&"coolmod".to_string()));
    }

    #[test]
    fn detects_wrong_java_and_missing_java() {
        assert_eq!(
            analyze("java.lang.UnsupportedClassVersionError: class file version 69.0").kind,
            CrashKind::WrongJavaVersion
        );
        assert_eq!(
            analyze("'java' is not recognized as an internal or external command").kind,
            CrashKind::MissingJava
        );
    }

    #[test]
    fn detects_bridge_failures() {
        let a = analyze("[Java mods] sodium failed in init: boom");
        assert_eq!(a.kind, CrashKind::BridgeInit);
    }

    #[test]
    fn unknown_logs_are_not_overstated() {
        let a = analyze("The game stopped unexpectedly.");
        assert_eq!(a.kind, CrashKind::Unknown);
        assert!(!a.confirmed);
        assert_eq!(a.confidence, 0);
        assert!(a.excerpt.is_empty());
    }
}
