// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Stack profile definitions — Group D: Java/Spring, .NET, Ruby on Rails,
//! Symfony/modern PHP, native mobile (Swift/Kotlin/Flutter).
//!
//! These close the cross-stack coverage gap: before P4 the curated profiles
//! were Rust/JS/Python/Go-centric, so a Java, C#, Ruby, PHP, or native-mobile
//! developer got no stack-tailored scoring (pain-point boosts, ecosystem-shift
//! rewards, competing-tech suppression). Each profile encodes the same curated
//! domain knowledge the existing eleven do.

use crate::stacks::{EcosystemShift, PainPoint, SeedItem, StackProfile};

// ============================================================================
// Java / Spring
// ============================================================================

pub static JAVA_ENTERPRISE: StackProfile = StackProfile {
    id: "java_enterprise",
    name: "Java / Spring",
    core_tech: &["java", "spring", "spring-boot", "kotlin"],
    companions: &[
        "hibernate",
        "maven",
        "gradle",
        "junit",
        "jackson",
        "lombok",
        "quarkus",
        "micronaut",
    ],
    competing: &["dotnet", "csharp", "node", "django", "rails", "go"],
    pain_points: &[
        PainPoint {
            keywords: &[
                "jvm",
                "garbage collection",
                "heap",
                "out of memory",
                "memory tuning",
                "gc pause",
            ],
            severity: 0.10,
            description: "JVM memory and GC tuning",
        },
        PainPoint {
            keywords: &[
                "maven",
                "gradle",
                "dependency",
                "classpath",
                "version conflict",
                "transitive",
            ],
            severity: 0.08,
            description: "Build and dependency management",
        },
        PainPoint {
            keywords: &[
                "startup time",
                "cold start",
                "native image",
                "graalvm",
                "boot time",
            ],
            severity: 0.10,
            description: "JVM startup latency",
        },
        PainPoint {
            keywords: &[
                "java 8",
                "java 17",
                "java 21",
                "jdk upgrade",
                "version migration",
            ],
            severity: 0.10,
            description: "JDK version migration",
        },
    ],
    ecosystem_shifts: &[
        EcosystemShift {
            from: "spring boot 2",
            to: "spring boot 3",
            keywords: &["spring boot 3", "spring 6", "spring framework 6"],
            boost: 1.15,
        },
        EcosystemShift {
            from: "javax",
            to: "jakarta",
            keywords: &["jakarta ee", "jakarta", "javax migration"],
            boost: 1.12,
        },
        EcosystemShift {
            from: "jvm",
            to: "graalvm native",
            keywords: &["graalvm", "native image", "native compilation"],
            boost: 1.12,
        },
        EcosystemShift {
            from: "junit 4",
            to: "junit 5",
            keywords: &["junit 5", "junit jupiter"],
            boost: 1.08,
        },
    ],
    keyword_boosts: &[
        ("spring", 0.12),
        ("spring-boot", 0.12),
        ("java", 0.10),
        ("kotlin", 0.08),
        ("hibernate", 0.07),
        ("quarkus", 0.08),
        ("jvm", 0.06),
    ],
    source_preferences: &[("reddit", 0.05), ("devto", 0.05)],
    detection_markers: &[
        "pom.xml",
        "build.gradle",
        "spring",
        "java",
        "maven",
        "gradle",
        "hibernate",
    ],
    detection_threshold: 2,
    seed_content: &[
        SeedItem {
            title: "Spring Blog",
            url: "https://spring.io/blog",
            source_type: "rss",
        },
        SeedItem {
            title: "Inside Java",
            url: "https://inside.java/",
            source_type: "rss",
        },
        SeedItem {
            title: "InfoQ Java",
            url: "https://www.infoq.com/java/",
            source_type: "rss",
        },
        SeedItem {
            title: "r/java",
            url: "https://www.reddit.com/r/java/",
            source_type: "reddit",
        },
    ],
};

// ============================================================================
// .NET / C#
// ============================================================================

pub static DOTNET: StackProfile = StackProfile {
    id: "dotnet",
    name: ".NET / C#",
    core_tech: &["dotnet", "csharp", "asp.net", "blazor"],
    companions: &[
        "entity framework",
        "ef core",
        "nuget",
        "xunit",
        "maui",
        "signalr",
        "dapper",
        "serilog",
    ],
    competing: &["java", "spring", "node", "django", "rails", "go"],
    pain_points: &[
        PainPoint {
            keywords: &[
                ".net framework",
                ".net core",
                "framework migration",
                "net 8",
                "net upgrade",
            ],
            severity: 0.12,
            description: ".NET Framework to modern .NET migration",
        },
        PainPoint {
            keywords: &[
                "async",
                "await",
                "deadlock",
                "configureawait",
                "synchronization context",
            ],
            severity: 0.08,
            description: "async/await pitfalls",
        },
        PainPoint {
            keywords: &[
                "entity framework",
                "ef core",
                "n+1",
                "query performance",
                "tracking",
            ],
            severity: 0.10,
            description: "EF Core query performance",
        },
        PainPoint {
            keywords: &[
                "aot",
                "trimming",
                "native aot",
                "startup",
                "assembly trimming",
            ],
            severity: 0.08,
            description: "AOT and trimming",
        },
    ],
    ecosystem_shifts: &[
        EcosystemShift {
            from: ".net framework",
            to: ".net 8",
            keywords: &[".net 8", ".net 9", "net core migration"],
            boost: 1.15,
        },
        EcosystemShift {
            from: "newtonsoft",
            to: "system.text.json",
            keywords: &["system.text.json", "newtonsoft migration"],
            boost: 1.10,
        },
        EcosystemShift {
            from: "mvc controllers",
            to: "minimal apis",
            keywords: &["minimal api", "minimal apis", "blazor"],
            boost: 1.12,
        },
    ],
    keyword_boosts: &[
        ("dotnet", 0.12),
        ("csharp", 0.12),
        ("asp.net", 0.10),
        ("blazor", 0.08),
        ("ef core", 0.08),
        ("maui", 0.07),
    ],
    source_preferences: &[("reddit", 0.05), ("devto", 0.05)],
    detection_markers: &[
        "csproj",
        ".csproj",
        "dotnet",
        "nuget",
        "asp.net",
        "blazor",
        "global.json",
    ],
    detection_threshold: 2,
    seed_content: &[
        SeedItem {
            title: ".NET Blog",
            url: "https://devblogs.microsoft.com/dotnet/",
            source_type: "rss",
        },
        SeedItem {
            title: "Andrew Lock | .NET Escapades",
            url: "https://andrewlock.net/",
            source_type: "rss",
        },
        SeedItem {
            title: "r/dotnet",
            url: "https://www.reddit.com/r/dotnet/",
            source_type: "reddit",
        },
    ],
};

// ============================================================================
// Ruby on Rails
// ============================================================================

pub static RUBY_RAILS: StackProfile = StackProfile {
    id: "ruby_rails",
    name: "Ruby on Rails",
    core_tech: &["ruby", "rails", "hotwire", "turbo"],
    companions: &[
        "sidekiq",
        "rspec",
        "activerecord",
        "stimulus",
        "puma",
        "kamal",
        "sorbet",
        "bundler",
    ],
    competing: &["django", "laravel", "php", "node", "spring", "dotnet"],
    pain_points: &[
        PainPoint {
            keywords: &[
                "rails upgrade",
                "rails 7",
                "rails 8",
                "deprecation",
                "version migration",
            ],
            severity: 0.10,
            description: "Rails version upgrade",
        },
        PainPoint {
            keywords: &["n+1", "activerecord", "eager loading", "query", "includes"],
            severity: 0.10,
            description: "ActiveRecord N+1 queries",
        },
        PainPoint {
            keywords: &[
                "asset pipeline",
                "sprockets",
                "importmap",
                "propshaft",
                "bundling",
            ],
            severity: 0.08,
            description: "Asset pipeline churn",
        },
        PainPoint {
            keywords: &["yjit", "ruby 3", "gc", "memory bloat", "ruby version"],
            severity: 0.08,
            description: "Ruby runtime and memory",
        },
    ],
    ecosystem_shifts: &[
        EcosystemShift {
            from: "webpacker",
            to: "importmap",
            keywords: &["importmap", "import map", "jsbundling", "esbuild rails"],
            boost: 1.12,
        },
        EcosystemShift {
            from: "sprockets",
            to: "propshaft",
            keywords: &["propshaft", "sprockets migration"],
            boost: 1.10,
        },
        EcosystemShift {
            from: "turbolinks",
            to: "hotwire",
            keywords: &["hotwire", "turbo", "stimulus", "turbo frames"],
            boost: 1.15,
        },
        EcosystemShift {
            from: "capistrano",
            to: "kamal",
            keywords: &["kamal", "mrsk", "kamal deploy"],
            boost: 1.10,
        },
    ],
    keyword_boosts: &[
        ("rails", 0.12),
        ("ruby", 0.10),
        ("hotwire", 0.08),
        ("turbo", 0.08),
        ("sidekiq", 0.07),
        ("activerecord", 0.07),
    ],
    source_preferences: &[("reddit", 0.05), ("devto", 0.05)],
    detection_markers: &[
        "Gemfile",
        "rails",
        "ruby",
        "activerecord",
        "bundler",
        ".ruby-version",
    ],
    detection_threshold: 2,
    seed_content: &[
        SeedItem {
            title: "Ruby on Rails Blog",
            url: "https://rubyonrails.org/blog",
            source_type: "rss",
        },
        SeedItem {
            title: "Ruby Weekly",
            url: "https://rubyweekly.com/",
            source_type: "rss",
        },
        SeedItem {
            title: "r/rails",
            url: "https://www.reddit.com/r/rails/",
            source_type: "reddit",
        },
    ],
};

// ============================================================================
// Symfony / modern PHP (Laravel is a separate profile)
// ============================================================================

pub static PHP_SYMFONY: StackProfile = StackProfile {
    id: "php_symfony",
    name: "Symfony / Modern PHP",
    core_tech: &["php", "symfony", "doctrine", "composer"],
    companions: &[
        "phpunit",
        "phpstan",
        "psr",
        "twig",
        "api platform",
        "pest",
        "rector",
        "psalm",
    ],
    competing: &["laravel", "rails", "django", "node", "spring", "dotnet"],
    pain_points: &[
        PainPoint {
            keywords: &[
                "php 7",
                "php 8",
                "php version",
                "deprecation",
                "php upgrade",
            ],
            severity: 0.10,
            description: "PHP version migration",
        },
        PainPoint {
            keywords: &[
                "strict types",
                "type juggling",
                "phpstan",
                "psalm",
                "static analysis",
            ],
            severity: 0.08,
            description: "Type safety and static analysis",
        },
        PainPoint {
            keywords: &[
                "composer",
                "dependency",
                "autoload",
                "version constraint",
                "psr-4",
            ],
            severity: 0.07,
            description: "Composer dependency management",
        },
        PainPoint {
            keywords: &["doctrine", "n+1", "hydration", "query", "orm performance"],
            severity: 0.08,
            description: "Doctrine ORM performance",
        },
    ],
    ecosystem_shifts: &[
        EcosystemShift {
            from: "php 7",
            to: "php 8",
            keywords: &["php 8.3", "php 8.4", "enums", "readonly", "fibers"],
            boost: 1.15,
        },
        EcosystemShift {
            from: "phpunit",
            to: "pest",
            keywords: &["pest", "pest php", "pest testing"],
            boost: 1.10,
        },
        EcosystemShift {
            from: "annotations",
            to: "attributes",
            keywords: &["php attributes", "annotations migration", "rector"],
            boost: 1.10,
        },
    ],
    keyword_boosts: &[
        ("symfony", 0.12),
        ("php", 0.10),
        ("doctrine", 0.08),
        ("phpstan", 0.07),
        ("twig", 0.06),
        ("composer", 0.06),
    ],
    source_preferences: &[("reddit", 0.05), ("devto", 0.05)],
    detection_markers: &[
        "composer.json",
        "symfony",
        "php",
        "doctrine",
        "psr",
        "phpstan.neon",
    ],
    detection_threshold: 2,
    seed_content: &[
        SeedItem {
            title: "Symfony Blog",
            url: "https://symfony.com/blog/",
            source_type: "rss",
        },
        SeedItem {
            title: "PHP.Watch",
            url: "https://php.watch/",
            source_type: "rss",
        },
        SeedItem {
            title: "r/PHP",
            url: "https://www.reddit.com/r/PHP/",
            source_type: "reddit",
        },
    ],
};

// ============================================================================
// Native mobile (Swift / Kotlin / Flutter) — distinct from React Native
// ============================================================================

pub static MOBILE_NATIVE: StackProfile = StackProfile {
    id: "mobile_native",
    name: "Native Mobile (iOS / Android / Flutter)",
    core_tech: &["swift", "kotlin", "flutter", "swiftui"],
    companions: &[
        "jetpack compose",
        "dart",
        "combine",
        "coroutines",
        "xcode",
        "android studio",
        "core data",
        "room",
    ],
    competing: &["react-native", "ionic", "cordova"],
    pain_points: &[
        PainPoint {
            keywords: &[
                "app store",
                "play store",
                "review",
                "rejection",
                "app review",
            ],
            severity: 0.08,
            description: "App store review and distribution",
        },
        PainPoint {
            keywords: &[
                "state management",
                "swiftui state",
                "compose state",
                "recomposition",
                "rebuild",
            ],
            severity: 0.10,
            description: "Mobile state management",
        },
        PainPoint {
            keywords: &[
                "platform",
                "fragmentation",
                "device",
                "screen size",
                "os version",
            ],
            severity: 0.08,
            description: "Platform fragmentation",
        },
        PainPoint {
            keywords: &[
                "build time",
                "xcode",
                "gradle build",
                "compile time",
                "ci build",
            ],
            severity: 0.08,
            description: "Mobile build times",
        },
    ],
    ecosystem_shifts: &[
        EcosystemShift {
            from: "uikit",
            to: "swiftui",
            keywords: &["swiftui", "swift ui", "uikit migration"],
            boost: 1.15,
        },
        EcosystemShift {
            from: "android views",
            to: "jetpack compose",
            keywords: &["jetpack compose", "compose multiplatform", "xml layout"],
            boost: 1.15,
        },
        EcosystemShift {
            from: "objective-c",
            to: "swift",
            keywords: &["swift migration", "objective-c bridging"],
            boost: 1.10,
        },
        EcosystemShift {
            from: "flutter skia",
            to: "flutter impeller",
            keywords: &["impeller", "flutter impeller", "skia migration"],
            boost: 1.08,
        },
    ],
    keyword_boosts: &[
        ("swift", 0.10),
        ("swiftui", 0.10),
        ("kotlin", 0.08),
        ("jetpack compose", 0.10),
        ("flutter", 0.10),
        ("dart", 0.07),
        ("ios", 0.06),
        ("android", 0.06),
    ],
    source_preferences: &[("reddit", 0.05), ("devto", 0.05)],
    detection_markers: &[
        "Package.swift",
        "pubspec.yaml",
        "swift",
        "kotlin",
        "swiftui",
        "flutter",
        "build.gradle.kts",
    ],
    detection_threshold: 2,
    seed_content: &[
        SeedItem {
            title: "Swift by Sundell",
            url: "https://swiftbysundell.com/",
            source_type: "rss",
        },
        SeedItem {
            title: "Android Developers Blog",
            url: "https://android-developers.googleblog.com/",
            source_type: "rss",
        },
        SeedItem {
            title: "Flutter",
            url: "https://medium.com/flutter",
            source_type: "rss",
        },
    ],
};
