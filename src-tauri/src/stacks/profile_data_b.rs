// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Stack profile definitions — Group B: React Native, Laravel, Django.

use crate::stacks::{EcosystemShift, PainPoint, SeedItem, StackProfile};

// ============================================================================
// React Native
// ============================================================================

pub static REACT_NATIVE: StackProfile = StackProfile {
    id: "react_native",
    name: "React Native / Expo",
    core_tech: &["react-native", "expo", "typescript", "react"],
    companions: &[
        "expo-router",
        "reanimated",
        "gesture-handler",
        "react-navigation",
        "zustand",
        "tanstack-query",
        "nativewind",
    ],
    competing: &["flutter", "kotlin", "swift", "ionic", "capacitor"],
    pain_points: &[
        PainPoint {
            keywords: &[
                "new architecture",
                "fabric",
                "turbo module",
                "bridgeless",
                "new arch",
            ],
            severity: 0.15,
            description: "New architecture migration",
        },
        PainPoint {
            keywords: &["hermes", "engine", "jsc", "javascript core", "hermes quirk"],
            severity: 0.10,
            description: "Hermes engine quirks",
        },
        PainPoint {
            keywords: &[
                "app store",
                "review",
                "rejection",
                "guideline",
                "app review",
            ],
            severity: 0.08,
            description: "App store review issues",
        },
        PainPoint {
            keywords: &[
                "ota",
                "over the air",
                "code push",
                "eas update",
                "expo update",
            ],
            severity: 0.10,
            description: "OTA update reliability",
        },
        PainPoint {
            keywords: &[
                "js thread",
                "ui thread",
                "performance",
                "frame drop",
                "jank",
            ],
            severity: 0.12,
            description: "JS thread performance",
        },
    ],
    ecosystem_shifts: &[
        EcosystemShift {
            from: "bare rn",
            to: "expo",
            keywords: &["expo", "expo go", "eas build", "expo managed", "expo sdk"],
            boost: 1.15,
        },
        EcosystemShift {
            from: "react-navigation",
            to: "expo-router",
            keywords: &["expo router", "file-based routing", "expo-router"],
            boost: 1.10,
        },
        EcosystemShift {
            from: "old arch",
            to: "new arch",
            keywords: &["new architecture", "fabric", "turbo module", "bridgeless"],
            boost: 1.15,
        },
    ],
    keyword_boosts: &[
        ("react native", 0.12),
        ("expo", 0.10),
        ("react-native", 0.12),
        ("mobile app", 0.06),
        ("eas build", 0.08),
        ("native module", 0.08),
    ],
    source_preferences: &[("reddit", 0.10), ("devto", 0.05)],
    detection_markers: &[
        "react-native",
        "expo",
        "app.json",
        "eas.json",
        "metro.config",
        "react-native.config",
    ],
    detection_threshold: 2,
    seed_content: &[
        SeedItem {
            title: "React Native Blog",
            url: "https://reactnative.dev/blog",
            source_type: "web",
        },
        SeedItem {
            title: "Expo Blog",
            url: "https://expo.dev/blog",
            source_type: "web",
        },
        SeedItem {
            title: "r/reactnative",
            url: "https://www.reddit.com/r/reactnative/",
            source_type: "reddit",
        },
        SeedItem {
            title: "React Native Newsletter",
            url: "https://reactnativenewsletter.com/",
            source_type: "rss",
        },
    ],
};

// ============================================================================
// Laravel
// ============================================================================

pub static LARAVEL: StackProfile = StackProfile {
    id: "laravel",
    name: "Laravel",
    core_tech: &["laravel", "php", "mysql", "redis"],
    companions: &[
        "livewire", "filament", "inertia", "blade", "pest", "forge", "vapor", "horizon", "sanctum",
    ],
    competing: &["symfony", "django", "rails", "express", "spring"],
    pain_points: &[
        PainPoint {
            keywords: &[
                "php version",
                "php 8",
                "php 7",
                "php compatibility",
                "php deprecation",
                "php upgrade",
            ],
            severity: 0.10,
            description: "PHP version migration",
        },
        PainPoint {
            keywords: &["queue", "job", "failed", "retry", "horizon", "worker"],
            severity: 0.12,
            description: "Queue and job reliability",
        },
        PainPoint {
            keywords: &[
                "n+1",
                "eager loading",
                "query",
                "eloquent performance",
                "lazy loading",
            ],
            severity: 0.10,
            description: "N+1 query problems",
        },
        PainPoint {
            keywords: &["deployment", "forge", "envoyer", "vapor", "docker"],
            severity: 0.08,
            description: "Deployment complexity",
        },
    ],
    ecosystem_shifts: &[
        EcosystemShift {
            from: "livewire 2",
            to: "livewire 3",
            keywords: &[
                "livewire 3",
                "livewire v3",
                "livewire upgrade",
                "wire:navigate",
            ],
            boost: 1.15,
        },
        EcosystemShift {
            from: "nova",
            to: "filament",
            keywords: &[
                "filament",
                "filament admin",
                "filament v3",
                "filament panel",
            ],
            boost: 1.15,
        },
        EcosystemShift {
            from: "phpunit",
            to: "pest",
            keywords: &["pest", "pest v3", "pest testing", "arch testing"],
            boost: 1.10,
        },
    ],
    keyword_boosts: &[
        ("laravel", 0.12),
        ("livewire", 0.10),
        ("eloquent", 0.08),
        ("blade", 0.06),
        ("filament", 0.08),
        ("inertia", 0.08),
    ],
    source_preferences: &[("reddit", 0.05), ("devto", 0.10)],
    detection_markers: &[
        "laravel",
        "artisan",
        "composer.json",
        "eloquent",
        "blade",
        "livewire",
    ],
    detection_threshold: 2,
    seed_content: &[
        SeedItem {
            title: "Laravel News",
            url: "https://laravel-news.com/",
            source_type: "rss",
        },
        SeedItem {
            title: "Laravel Blog",
            url: "https://blog.laravel.com/",
            source_type: "web",
        },
        SeedItem {
            title: "r/laravel",
            url: "https://www.reddit.com/r/laravel/",
            source_type: "reddit",
        },
        SeedItem {
            title: "Laracasts Discussions",
            url: "https://laracasts.com/discuss",
            source_type: "web",
        },
    ],
};

// ============================================================================
// Django
// ============================================================================

pub static DJANGO: StackProfile = StackProfile {
    id: "django",
    name: "Django",
    core_tech: &["django", "python", "postgresql", "celery"],
    companions: &[
        "drf",
        "django-rest-framework",
        "wagtail",
        "htmx",
        "django-ninja",
        "gunicorn",
        "pytest-django",
        "redis",
    ],
    competing: &["flask", "fastapi", "rails", "laravel", "express"],
    pain_points: &[
        PainPoint {
            keywords: &[
                "orm",
                "queryset",
                "n+1",
                "select_related",
                "prefetch_related",
            ],
            severity: 0.12,
            description: "ORM performance",
        },
        PainPoint {
            keywords: &["async", "asgi", "channels", "async view", "django async"],
            severity: 0.10,
            description: "Async support",
        },
        PainPoint {
            keywords: &["migration", "conflict", "merge", "squash", "makemigrations"],
            severity: 0.10,
            description: "Migration conflicts",
        },
        PainPoint {
            keywords: &[
                "test speed",
                "pytest",
                "fixture",
                "factory",
                "test database",
            ],
            severity: 0.08,
            description: "Test suite speed",
        },
    ],
    ecosystem_shifts: &[
        EcosystemShift {
            from: "drf",
            to: "django-ninja",
            keywords: &["django-ninja", "ninja api", "pydantic", "django ninja"],
            boost: 1.15,
        },
        EcosystemShift {
            from: "javascript",
            to: "htmx",
            keywords: &[
                "htmx",
                "hypermedia",
                "hx-get",
                "hx-post",
                "html over the wire",
            ],
            boost: 1.15,
        },
        EcosystemShift {
            from: "custom cms",
            to: "wagtail",
            keywords: &["wagtail", "wagtail cms", "streamfield", "wagtail page"],
            boost: 1.10,
        },
    ],
    keyword_boosts: &[
        ("django", 0.12),
        ("drf", 0.08),
        ("celery", 0.08),
        ("htmx", 0.08),
        ("wagtail", 0.06),
        ("django-ninja", 0.08),
    ],
    source_preferences: &[("reddit", 0.05), ("hackernews", 0.05)],
    detection_markers: &[
        "django",
        "manage.py",
        "settings.py",
        "celery",
        "drf",
        "wagtail",
    ],
    detection_threshold: 2,
    seed_content: &[
        SeedItem {
            title: "Django Project Blog",
            url: "https://www.djangoproject.com/weblog/",
            source_type: "web",
        },
        SeedItem {
            title: "Django Weekly",
            url: "https://django-news.com/",
            source_type: "rss",
        },
        SeedItem {
            title: "r/django",
            url: "https://www.reddit.com/r/django/",
            source_type: "reddit",
        },
        SeedItem {
            title: "Real Python Django",
            url: "https://realpython.com/tutorials/django/",
            source_type: "web",
        },
    ],
};
