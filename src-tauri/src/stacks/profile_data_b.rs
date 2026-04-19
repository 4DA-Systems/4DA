// SPDX-License-Identifier: FSL-1.1-Apache-2.0
//! Stack profile definitions — Group B: React Native, Laravel, Django, Vue,
//! DevOps/SRE, Haskell/FP, Bootstrap Web Dev.

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

// ============================================================================
// Vue Frontend
// ============================================================================

pub static VUE_FRONTEND: StackProfile = StackProfile {
    id: "vue_frontend",
    name: "Vue / Nuxt",
    core_tech: &["vue", "nuxt", "pinia", "vite"],
    companions: &[
        "vuetify",
        "primevue",
        "unocss",
        "vitest",
        "vue-router",
        "vueuse",
        "tanstack-query",
    ],
    competing: &["react", "angular", "svelte", "solid"],
    pain_points: &[
        PainPoint {
            keywords: &[
                "composition api",
                "options api",
                "migration",
                "setup",
                "script setup",
            ],
            severity: 0.12,
            description: "Composition API migration",
        },
        PainPoint {
            keywords: &["ssr", "hydration", "mismatch", "nuxt ssr", "server render"],
            severity: 0.10,
            description: "SSR hydration mismatches",
        },
        PainPoint {
            keywords: &[
                "typescript",
                "definecomponent",
                "vue typescript",
                "vue types",
                "script setup",
            ],
            severity: 0.08,
            description: "TypeScript integration",
        },
        PainPoint {
            keywords: &[
                "vuex",
                "pinia",
                "state management",
                "store migration",
                "vuex to pinia",
            ],
            severity: 0.10,
            description: "Vuex to Pinia migration",
        },
    ],
    ecosystem_shifts: &[
        EcosystemShift {
            from: "vue 3",
            to: "vue vapor",
            keywords: &["vue vapor", "vapor mode", "compile-time", "no virtual dom"],
            boost: 1.20,
        },
        EcosystemShift {
            from: "nuxt 3",
            to: "nuxt 4",
            keywords: &["nuxt 4", "nuxt upgrade", "nuxt migration", "nuxt next"],
            boost: 1.15,
        },
        EcosystemShift {
            from: "tailwind",
            to: "unocss",
            keywords: &["unocss", "uno css", "atomic css", "unocss preset"],
            boost: 1.10,
        },
    ],
    keyword_boosts: &[
        ("vue", 0.10),
        ("nuxt", 0.10),
        ("pinia", 0.08),
        ("composition api", 0.08),
        ("vue vapor", 0.10),
        ("vueuse", 0.06),
    ],
    source_preferences: &[("devto", 0.10), ("reddit", 0.05)],
    detection_markers: &["vue", "nuxt", "pinia", "nuxt.config", "vite.config", ".vue"],
    detection_threshold: 2,
    seed_content: &[
        SeedItem {
            title: "Vue.js Blog",
            url: "https://blog.vuejs.org/",
            source_type: "web",
        },
        SeedItem {
            title: "Nuxt Blog",
            url: "https://nuxt.com/blog",
            source_type: "web",
        },
        SeedItem {
            title: "Vue.js News",
            url: "https://news.vuejs.org/",
            source_type: "rss",
        },
        SeedItem {
            title: "r/vuejs",
            url: "https://www.reddit.com/r/vuejs/",
            source_type: "reddit",
        },
    ],
};

// ============================================================================
// DevOps & SRE
// ============================================================================

pub static DEVOPS_SRE: StackProfile = StackProfile {
    id: "devops_sre",
    name: "DevOps & SRE",
    core_tech: &["kubernetes", "docker", "terraform", "ansible"],
    companions: &[
        "helm",
        "prometheus",
        "grafana",
        "istio",
        "argocd",
        "vault",
        "etcd",
        "cilium",
        "envoy",
        "datadog",
    ],
    competing: &["heroku", "railway", "render"],
    pain_points: &[
        PainPoint {
            keywords: &["cluster", "upgrade", "etcd", "control plane"],
            severity: 0.15,
            description: "Cluster lifecycle management",
        },
        PainPoint {
            keywords: &[
                "observability",
                "metrics",
                "tracing",
                "logging",
                "opentelemetry",
            ],
            severity: 0.12,
            description: "Observability stack complexity",
        },
        PainPoint {
            keywords: &["rbac", "network policy", "pod security", "admission"],
            severity: 0.10,
            description: "Security policy management",
        },
        PainPoint {
            keywords: &["terraform", "state", "drift", "plan", "apply"],
            severity: 0.12,
            description: "IaC state management",
        },
        PainPoint {
            keywords: &["ci", "cd", "pipeline", "deploy", "rollback", "canary"],
            severity: 0.10,
            description: "CI/CD pipeline reliability",
        },
    ],
    ecosystem_shifts: &[
        EcosystemShift {
            from: "helm",
            to: "kustomize",
            keywords: &["kustomize", "helm to kustomize", "kustomization"],
            boost: 1.12,
        },
        EcosystemShift {
            from: "jenkins",
            to: "github actions",
            keywords: &["github actions", "actions workflow", "jenkins migration"],
            boost: 1.10,
        },
        EcosystemShift {
            from: "nagios",
            to: "prometheus",
            keywords: &["prometheus migration", "alertmanager", "prometheus stack"],
            boost: 1.10,
        },
        EcosystemShift {
            from: "terraform",
            to: "pulumi",
            keywords: &["pulumi", "terraform to pulumi", "infrastructure sdk"],
            boost: 1.08,
        },
    ],
    keyword_boosts: &[
        ("kubernetes", 0.12),
        ("k8s", 0.12),
        ("docker", 0.10),
        ("terraform", 0.10),
        ("helm", 0.08),
        ("prometheus", 0.08),
        ("grafana", 0.06),
        ("ansible", 0.06),
        ("argocd", 0.08),
        ("istio", 0.08),
        ("observability", 0.08),
        ("sre", 0.06),
    ],
    source_preferences: &[("hackernews", 0.05), ("reddit", 0.05)],
    detection_markers: &[
        "kubernetes",
        "kubectl",
        "docker",
        "terraform",
        "helm",
        "prometheus",
        "k8s",
    ],
    detection_threshold: 2,
    seed_content: &[
        SeedItem {
            title: "Kubernetes Blog",
            url: "https://kubernetes.io/blog/",
            source_type: "web",
        },
        SeedItem {
            title: "CNCF Blog",
            url: "https://www.cncf.io/blog/",
            source_type: "web",
        },
        SeedItem {
            title: "HashiCorp Blog",
            url: "https://www.hashicorp.com/blog",
            source_type: "web",
        },
        SeedItem {
            title: "r/devops",
            url: "https://www.reddit.com/r/devops/",
            source_type: "reddit",
        },
        SeedItem {
            title: "SRE Weekly",
            url: "https://sreweekly.com/",
            source_type: "rss",
        },
    ],
};

// ============================================================================
// Haskell & Functional Programming
// ============================================================================

pub static HASKELL_FP: StackProfile = StackProfile {
    id: "haskell",
    name: "Haskell & Functional Programming",
    core_tech: &["haskell", "nix", "ghc", "cabal", "stack"],
    companions: &[
        "purescript",
        "ocaml",
        "elm",
        "agda",
        "idris",
        "coq",
        "lens",
        "mtl",
        "servant",
        "yesod",
        "pandoc",
    ],
    competing: &[],
    pain_points: &[
        PainPoint {
            keywords: &["ghc", "upgrade", "breaking", "version", "migration"],
            severity: 0.12,
            description: "GHC version upgrades",
        },
        PainPoint {
            keywords: &["cabal", "stack", "dependency", "resolver", "build"],
            severity: 0.10,
            description: "Build tool fragmentation",
        },
        PainPoint {
            keywords: &["monad", "transformer", "effect", "mtl", "io"],
            severity: 0.10,
            description: "Effect system complexity",
        },
        PainPoint {
            keywords: &["nix", "flake", "derivation", "nixpkgs", "nixos"],
            severity: 0.10,
            description: "Nix ecosystem complexity",
        },
    ],
    ecosystem_shifts: &[
        EcosystemShift {
            from: "mtl",
            to: "effectful",
            keywords: &["effectful", "effect system", "mtl alternative"],
            boost: 1.12,
        },
        EcosystemShift {
            from: "cabal",
            to: "cabal+nix",
            keywords: &["nix flake", "haskell.nix", "cabal2nix"],
            boost: 1.08,
        },
    ],
    keyword_boosts: &[
        ("haskell", 0.12),
        ("ghc", 0.10),
        ("cabal", 0.08),
        ("nix", 0.08),
        ("functional programming", 0.10),
        ("type theory", 0.08),
        ("category theory", 0.06),
        ("monad", 0.08),
        ("algebraic", 0.06),
        ("purescript", 0.06),
        ("ocaml", 0.06),
    ],
    source_preferences: &[("hackernews", 0.05), ("lobsters", 0.15)],
    detection_markers: &[
        "haskell",
        "ghc",
        "cabal",
        "stack.yaml",
        ".cabal",
        "nix",
        "flake.nix",
    ],
    detection_threshold: 2,
    seed_content: &[
        SeedItem {
            title: "Haskell Weekly",
            url: "https://haskellweekly.news/",
            source_type: "rss",
        },
        SeedItem {
            title: "r/haskell",
            url: "https://www.reddit.com/r/haskell/",
            source_type: "reddit",
        },
        SeedItem {
            title: "Haskell Planet",
            url: "https://planet.haskell.org/",
            source_type: "rss",
        },
        SeedItem {
            title: "NixOS Discourse",
            url: "https://discourse.nixos.org/",
            source_type: "web",
        },
    ],
};

// ============================================================================
// General Web Development (Bootstrap)
// ============================================================================

pub static BOOTSTRAP_WEBDEV: StackProfile = StackProfile {
    id: "bootstrap_webdev",
    name: "General Web Development",
    core_tech: &["typescript", "javascript", "react", "nodejs"],
    companions: &[
        "css",
        "html",
        "vite",
        "webpack",
        "eslint",
        "prettier",
        "tailwindcss",
        "nextjs",
        "express",
        "npm",
    ],
    competing: &[],
    pain_points: &[
        PainPoint {
            keywords: &["dependency", "npm", "package", "breaking change", "upgrade"],
            severity: 0.10,
            description: "Package ecosystem churn",
        },
        PainPoint {
            keywords: &["build", "bundle", "webpack", "vite", "config"],
            severity: 0.08,
            description: "Build tooling complexity",
        },
        PainPoint {
            keywords: &["typescript", "type", "inference", "strict", "any"],
            severity: 0.08,
            description: "Type system adoption",
        },
    ],
    ecosystem_shifts: &[
        EcosystemShift {
            from: "webpack",
            to: "vite",
            keywords: &["vite", "vite migration", "webpack to vite"],
            boost: 1.10,
        },
        EcosystemShift {
            from: "create-react-app",
            to: "nextjs",
            keywords: &["nextjs", "cra migration", "app router"],
            boost: 1.08,
        },
    ],
    keyword_boosts: &[("typescript", 0.10), ("javascript", 0.06)],
    source_preferences: &[("hackernews", 0.05)],
    detection_markers: &[
        "typescript",
        "javascript",
        "react",
        "package.json",
        "tsconfig",
        "node_modules",
        "npm",
    ],
    detection_threshold: 2,
    seed_content: &[
        SeedItem {
            title: "JavaScript Weekly",
            url: "https://javascriptweekly.com/",
            source_type: "rss",
        },
        SeedItem {
            title: "React Blog",
            url: "https://react.dev/blog",
            source_type: "web",
        },
        SeedItem {
            title: "Node.js Blog",
            url: "https://nodejs.org/en/blog",
            source_type: "web",
        },
        SeedItem {
            title: "r/javascript",
            url: "https://www.reddit.com/r/javascript/",
            source_type: "reddit",
        },
    ],
};
