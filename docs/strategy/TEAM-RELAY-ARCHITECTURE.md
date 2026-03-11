# 4DA Team Relay — Coordination Architecture

**Status:** Design complete, ready for implementation
**Purpose:** This is the missing foundation that both TEAM-TIER-PLAN.md and ENTERPRISE-TIER-PLAN.md depend on. No multi-seat feature can exist without this layer.

---

## The Problem

4DA is a desktop app. Each seat is an independent Tauri instance with its own SQLite database on a different machine. Team and Enterprise features require sharing metadata (tech stacks, scores, signal types, decisions) between seats. Raw content and code never leave the machine — only lightweight metadata transits.

## The Solution: Dumb Relay, Smart Clients

A thin, encrypted metadata relay server. The relay stores and routes opaque encrypted blobs indexed by `(team_id, client_id, sequence_number)`. It cannot read any metadata — all encryption happens client-side. This is the same architecture proven by Obsidian Sync, Linear, and Syncthing.

**Key properties:**
- Relay sees only ciphertext — structurally cannot access team intelligence
- Clients are authoritative for their own state — relay just routes
- Offline-tolerant — clients sync on next launch via `?since={seq}`
- Self-hostable — enterprise customers run the relay on their own infrastructure

---

## Architecture Overview

```
CLIENT A (4DA desktop)                  RELAY SERVER                  CLIENT B (4DA desktop)
     |                                       |                              |
     | 1. Local event occurs                 |                              |
     |    (signal detected, DNA updated)     |                              |
     |                                       |                              |
     | 2. Create TeamMetadataEntry           |                              |
     |    Encrypt with shared team key       |                              |
     |    Queue in local sync_queue          |                              |
     |                                       |                              |
     | 3. POST /teams/{id}/entries --------> |                              |
     |    (encrypted blob)                   | Store blob                   |
     |                          <--- ACK --- | Assign relay_seq             |
     |                                       |                              |
     |                                       | 4. SSE: "new entry" -------> |
     |                                       |                              |
     |                                       | <--- GET /entries?since=N -- |
     |                                       | --- return encrypted blobs -> |
     |                                       |                              |
     |                                       |              5. Decrypt blob |
     |                                       |    Apply to local team tables |
     |                                       |                 Update UI    |
```

---

## What Gets Synced (and What Doesn't)

### SYNCED (metadata only, ~200-500 bytes per operation):
- Tech stack names + scores (not code, not file paths)
- Signal chain metadata (type, severity, title — not source content)
- Decision records (title, status, vote — not the content that informed them)
- Developer DNA summary (stack, interests, blind spots — not project paths)
- Team membership changes (join, leave, role change)

### NEVER SYNCED:
- Source content (articles, RSS items, HN posts)
- Raw code or file contents
- File system paths or directory structures
- API keys or credentials
- Embedding vectors
- Local database state

---

## Encryption: XChaCha20Poly1305 + X25519

### Why This Stack
- Same cryptographic primitives as WireGuard, Signal, and Borg Backup
- Pure Rust — no C dependencies (matches 4DA's existing philosophy: ocrs over tesseract)
- XChaCha20's 192-bit nonce means random nonces are collision-safe across millions of messages
- Authenticated encryption — tampered blobs fail decryption, not silently corrupt

### Crates (all pure Rust, no FFI)
```toml
chacha20poly1305 = "0.10"   # AEAD cipher
x25519-dalek = { version = "2", features = ["static_secrets"] }  # Key exchange
hkdf = "0.12"               # Key derivation from shared secret
# sha2 = "0.10"             # Already in Cargo.toml
# ed25519-dalek = "2.2.0"   # Already in Cargo.toml (for relay auth JWT)
```

### Key Bootstrap Flow
1. Admin creates team → generates X25519 keypair → public key stored on relay
2. Admin generates invite code embedding `(team_id, admin_public_key, hmac)`
3. Member joins → generates own keypair → exchanges public keys via relay
4. Both sides derive shared secret via X25519 Diffie-Hellman
5. Shared secret expanded via HKDF into encryption key
6. All subsequent metadata encrypted with team-wide symmetric key

### Team Key Derivation
```
team_key = HKDF-SHA256(
    salt: team_id bytes,
    ikm: admin_keypair.diffie_hellman(member_keypair),
    info: "4da-team-sync-v1"
)
```

For teams with >2 members: admin derives a team-wide symmetric key, encrypts it with each member's X25519 public key, and distributes via the relay. New members receive the team key encrypted to their public key during the join flow.

---

## Conflict Resolution: Last Write Wins with HLC

For metadata (scores, signal types, tech names), the last person to edit wins. This is the same model Linear uses. CRDTs are overkill for key-value metadata — they earn their complexity for collaborative text editing, which 4DA doesn't do.

### Hybrid Logical Clock
```toml
uhlc = "0.7"  # Unique Hybrid Logical Clock — constant-size timestamps
```

Each client maintains an HLC that combines wall clock time with a logical counter. When two clients edit the same field:
- Compare HLC timestamps
- Higher timestamp wins
- No coordination needed — pure local merge

When receiving remote entries, update local HLC to maintain causal ordering.

---

## Transport: WebSocket + HTTP Polling Fallback

### Primary: WebSocket (real-time when app is open)
```toml
tokio-tungstenite = { version = "0.23", features = ["rustls-tls-webpki-roots"] }
```

Client connects to `wss://relay.4da.ai/teams/{id}/ws` on app launch. Relay broadcasts new entries in real-time. Connection drops gracefully when app closes.

### Fallback: HTTP Polling (catch-up after offline)
Uses existing `reqwest 0.12` — no new dependency. On app launch:
```
GET /teams/{id}/entries?since={last_known_seq}
```
Returns all entries missed while offline. Simple, reliable, works across any network.

### SSE for Notifications (lightweight alternative)
Relay sends Server-Sent Events with sequence numbers only — no payload. Client fetches actual encrypted blobs via HTTP. This way the notification channel carries zero sensitive data.

---

## Relay Server Design

### Technology
- **Language:** Rust (Axum 0.7)
- **Database:** SQLite (embedded, for small teams) or PostgreSQL (for managed/large)
- **Auth:** JWT tokens scoped to team_id, issued from Keygen license validation
- **Deployment:** Single static binary, Docker image, or managed at relay.4da.ai
- **Size:** ~400-600 lines of Rust for the complete relay

### API Surface (6 endpoints — this is everything)
```
POST   /auth/team                         # Exchange Keygen license → JWT
POST   /teams/{team_id}/entries           # Push encrypted changeset
GET    /teams/{team_id}/entries?since=N   # Pull changesets since sequence N
GET    /teams/{team_id}/clients           # List registered team members
POST   /teams/{team_id}/clients           # Register/heartbeat a client
GET    /teams/{team_id}/stream            # SSE notification stream
```

### Relay Schema (on the server)
```sql
CREATE TABLE sync_entries (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    team_id     TEXT NOT NULL,
    client_id   TEXT NOT NULL,
    seq         INTEGER NOT NULL,
    payload     BLOB NOT NULL,              -- encrypted, relay cannot read
    created_at  TEXT DEFAULT (datetime('now')),
    UNIQUE (team_id, client_id, seq)
);

CREATE TABLE team_clients (
    team_id      TEXT NOT NULL,
    client_id    TEXT NOT NULL,
    public_key   BLOB NOT NULL,             -- X25519 public key
    display_name TEXT,
    last_seen    TEXT,
    PRIMARY KEY (team_id, client_id)
);

CREATE INDEX idx_sync_team_seq ON sync_entries(team_id, id DESC);
```

### Relay Crates
```toml
axum = "0.7"
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
sqlx = { version = "0.8", features = ["sqlite", "runtime-tokio-rustls"] }
tower-http = { version = "0.5", features = ["cors", "trace", "compression-gzip"] }
jsonwebtoken = "9"
uuid = { version = "1", features = ["v4", "serde"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tracing = "0.1"
tracing-subscriber = "0.3"
```

### Self-Hosted (Enterprise)
```yaml
# docker-compose.yml
services:
  relay:
    image: ghcr.io/4da-systems/relay:latest
    ports: ["8443:8443"]
    environment:
      DATABASE_URL: "sqlite:///data/relay.db"
      JWT_SECRET: "${RELAY_JWT_SECRET}"
    volumes:
      - relay_data:/data
    restart: unless-stopped
volumes:
  relay_data:
```

Enterprise customers point their 4DA instances to `https://relay.company.internal:8443`. Data never leaves their infrastructure.

---

## Client-Side Schema (on each 4DA desktop)

Added to the existing SQLite database via new migration phases:

```sql
-- Phase 27a: Team sync infrastructure
CREATE TABLE IF NOT EXISTS team_sync_queue (
    entry_id    TEXT PRIMARY KEY,
    operation   TEXT NOT NULL,               -- JSON: TeamOp
    hlc_ts      INTEGER NOT NULL,            -- HLC timestamp
    encrypted   BLOB,                        -- encrypted payload (NULL until encrypted)
    relay_seq   INTEGER,                     -- assigned by relay after ACK
    created_at  INTEGER NOT NULL,
    acked_at    INTEGER                      -- NULL until relay confirms
);

CREATE TABLE IF NOT EXISTS team_sync_log (
    relay_seq   INTEGER NOT NULL,
    team_id     TEXT NOT NULL,
    client_id   TEXT NOT NULL,
    encrypted   BLOB NOT NULL,
    received_at INTEGER NOT NULL,
    applied     INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (relay_seq, team_id)
);

CREATE TABLE IF NOT EXISTS team_sync_state (
    team_id         TEXT PRIMARY KEY,
    last_relay_seq  INTEGER NOT NULL DEFAULT 0,
    last_sync_at    INTEGER
);

CREATE TABLE IF NOT EXISTS team_crypto (
    team_id         TEXT PRIMARY KEY,
    our_public_key  BLOB NOT NULL,
    our_private_key BLOB NOT NULL,           -- encrypted at rest with machine key
    team_symmetric_key BLOB,                 -- encrypted, for >2 member teams
    created_at      INTEGER NOT NULL
);
```

---

## Sync Payload Types

```rust
#[derive(Serialize, Deserialize, TS)]
#[ts(export)]
pub struct TeamMetadataEntry {
    pub entry_id: String,           // UUID, idempotency key
    pub client_id: String,          // which team member
    pub hlc_timestamp: u64,         // HLC for LWW ordering
    pub operation: TeamOp,          // what changed
}

#[derive(Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(tag = "type")]
pub enum TeamOp {
    // --- DNA sharing ---
    ShareDnaSummary {
        primary_stack: Vec<String>,
        interests: Vec<String>,
        blind_spots: Vec<String>,       // topic names only, no paths
        identity_summary: String,
    },

    // --- Signal chains ---
    ShareSignal {
        signal_id: String,
        chain_name: String,
        priority: String,               // "critical" | "high" | "medium" | "low"
        tech_topics: Vec<String>,
        suggested_action: String,
    },
    ResolveSignal {
        signal_id: String,
        resolution_notes: String,
    },

    // --- Decisions ---
    ProposeDecision {
        decision_id: String,
        title: String,
        decision_type: String,
        rationale: String,
    },
    VoteOnDecision {
        decision_id: String,
        stance: String,                 // "for" | "against" | "abstain"
        rationale: String,
    },

    // --- Context ---
    ShareContextSummary {
        active_topics: Vec<String>,
        tech_scores: Vec<(String, f32)>,
    },

    // --- Team membership ---
    MemberJoined {
        display_name: String,
        role: String,
    },
    MemberLeft {
        reason: String,
    },
    RoleChanged {
        new_role: String,
    },
}
```

Each `TeamOp` serialized is ~200-500 bytes. Encrypted blob is ~250-600 bytes. An active team's daily traffic is ~100 operations = ~60KB. Trivial bandwidth.

---

## Invite Flow (Server-Assisted)

The audit correctly identified that invite codes need server-side coordination. Here's how it works:

### Admin Creates Invite
1. Admin hits `POST /teams/{team_id}/invites` with `{email, role, expires_in: 72h}`
2. Relay generates invite record with unique code, stores it server-side
3. Relay returns invite code: `4DA-TEAM-{base64(relay_generated_token)}`
4. Admin shares code via email/Slack/chat (out-of-band)

### Member Joins
1. Member enters invite code in 4DA settings
2. Client hits `POST /auth/invite` with `{code}`
3. Relay validates: code exists, not expired, not used, email matches (optional)
4. Relay marks invite as consumed (single-use enforced server-side)
5. Relay returns: `{team_id, jwt_token, admin_public_key}`
6. Client generates X25519 keypair, registers public key on relay
7. Client derives shared encryption key from admin's public key
8. Client receives team symmetric key (encrypted to their public key)

### Why This Works
- Single-use enforcement lives on the relay (source of truth)
- Invite codes are opaque tokens (no embedded email addresses)
- Key exchange happens via relay's public key registry
- Team ID comes from the relay, not embedded in the invite

---

## Background Sync Scheduler

Integrates with the existing monitoring scheduler pattern from `src-tauri/src/monitoring.rs`:

```rust
pub fn start_team_sync_scheduler<R: Runtime>(app: AppHandle<R>, state: Arc<TeamSyncState>) {
    tauri::async_runtime::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(30));

        loop {
            interval.tick().await;

            if !state.enabled.load(Ordering::Relaxed) {
                continue;
            }

            // 1. Push any queued local entries to relay
            if let Err(e) = push_pending_entries(&state).await {
                tracing::warn!(target: "4da::team_sync", "Push failed: {e}");
            }

            // 2. Pull any new entries from relay
            if let Err(e) = pull_new_entries(&state).await {
                tracing::warn!(target: "4da::team_sync", "Pull failed: {e}");
            }

            // 3. Apply unapplied entries to local team tables
            if let Err(e) = apply_pending_entries(&state).await {
                tracing::warn!(target: "4da::team_sync", "Apply failed: {e}");
            }
        }
    });
}
```

On app launch: immediate full sync (`?since=last_relay_seq`).
While running: 30-second polling interval + WebSocket for real-time.
On app close: graceful — any unsynced entries persist in `team_sync_queue` for next launch.

---

## Privacy Guarantees

| Data | Leaves machine? | Encrypted? | Relay can read? |
|------|-----------------|------------|-----------------|
| Source content (articles, posts) | Never | N/A | N/A |
| Raw code / file paths | Never | N/A | N/A |
| API keys / credentials | Never | N/A | N/A |
| Embedding vectors | Never | N/A | N/A |
| Tech stack names | Yes (as metadata) | Yes (E2E) | No |
| Signal severity/type | Yes (as metadata) | Yes (E2E) | No |
| Decision titles/votes | Yes (as metadata) | Yes (E2E) | No |
| DNA summary (no paths) | Yes (as metadata) | Yes (E2E) | No |
| Team membership changes | Yes | Yes (E2E) | No |

The relay is structurally unable to access team intelligence. Even if compromised, an attacker gets only encrypted blobs with no key material.

---

## Existing Infrastructure We Reuse

| Component | File | What We Reuse |
|-----------|------|---------------|
| HTTP client | `src-tauri/src/http_client.rs` | reqwest 0.12 singleton pattern |
| Background scheduler | `src-tauri/src/monitoring.rs` | tokio interval + atomic state pattern |
| Settings persistence | `src-tauri/src/settings/mod.rs` | SettingsManager + serde defaults |
| License validation | `src-tauri/src/settings/license.rs` | Keygen API call pattern + caching |
| Anonymous ID | `src-tauri/src/community_intelligence.rs` | SHA-256 ID generation |
| Email transport | `src-tauri/src/digest_email.rs` | Async external communication pattern |
| Ed25519 signing | `src-tauri/src/settings/license.rs` | ed25519-dalek already in Cargo.toml |
| SHA-256 hashing | `Cargo.toml` | sha2 0.10 already available |

---

## New Crates Required (Client)

```toml
# Add to src-tauri/Cargo.toml
chacha20poly1305 = "0.10"                                           # E2E encryption
x25519-dalek = { version = "2", features = ["static_secrets"] }     # Key exchange
hkdf = "0.12"                                                       # Key derivation
uhlc = "0.7"                                                        # Hybrid logical clock
tokio-tungstenite = { version = "0.23", features = ["rustls-tls-webpki-roots"] }  # WebSocket
```

All pure Rust. No C dependencies. No new async runtime.

---

## New Files Required (Client)

```
src-tauri/src/
  team_sync.rs            # Core sync engine (push/pull/apply)
  team_sync_crypto.rs     # Encryption, key exchange, key management
  team_sync_types.rs      # TeamMetadataEntry, TeamOp, sync state types
  team_sync_scheduler.rs  # Background sync scheduler
  team_sync_commands.rs   # Tauri commands for team sync UI
  team_sync_tests.rs      # Unit tests for sync + crypto + conflict resolution
```

### Relay Server (new workspace member)

```
relay/
  Cargo.toml
  src/
    main.rs               # Axum server setup + routes
    auth.rs               # JWT issuance + validation
    entries.rs            # Sync entry storage + retrieval
    clients.rs            # Client registration + heartbeat
    stream.rs             # SSE notification stream
    db.rs                 # SQLx schema + queries
    error.rs              # Error types
  Dockerfile
  docker-compose.yml
```

---

## Implementation Phases

### Phase 1: Local Sync Primitives (no server yet)
- Add `uhlc` crate, create `TeamMetadataEntry` and `TeamOp` types
- Create `team_sync_queue`, `team_sync_log`, `team_sync_state` tables
- Tauri commands: `queue_team_entry`, `get_pending_entries`, `apply_entry`
- Unit tests for HLC ordering, LWW conflict resolution, entry queuing
- **Deliverable:** Local sync infrastructure that queues operations — ready for a relay

### Phase 2: Encryption Layer
- Add `chacha20poly1305`, `x25519-dalek`, `hkdf` crates
- Implement `TeamCrypto` in `team_sync_crypto.rs`
- Key generation, X25519 key exchange, HKDF derivation
- Encrypt/decrypt round-trip tests
- Team key distribution for >2 members
- **Deliverable:** All metadata encrypted before leaving the machine

### Phase 3: Relay Server
- New `relay/` workspace member with Axum
- All 6 API endpoints
- SQLite storage for sync entries + client registry
- JWT auth tied to Keygen license validation
- Rate limiting via tower-http
- Integration tests (client ↔ relay round-trip)
- **Deliverable:** Working relay that stores and routes encrypted blobs

### Phase 4: Client-Server Integration
- WebSocket connection from 4DA client to relay
- HTTP polling fallback for offline catch-up
- Background sync scheduler (30s interval)
- SSE notification listener for real-time updates
- Graceful offline handling (queue locally, sync on reconnect)
- **Deliverable:** Two 4DA instances syncing metadata in real-time

### Phase 5: Invite & Identity
- Server-side invite generation + consumption
- Client join flow (enter code → exchange keys → sync team state)
- Member list management (join, leave, role change)
- Admin controls (generate invites, remove members)
- **Deliverable:** Complete team formation flow

### Phase 6: Self-Hosted Packaging
- Dockerfile + docker-compose.yml for relay
- SQLite mode (default, zero-config) and PostgreSQL mode (configurable)
- Environment variable configuration
- Health check endpoint (`GET /health`)
- **Deliverable:** Enterprise customers can self-host with `docker compose up`

---

## How This Enables Team and Enterprise Plans

With the relay in place, every feature in both plans becomes tractable:

| Feature | How It Works With Relay |
|---------|------------------------|
| Team DNA Dashboard | Each seat syncs `ShareDnaSummary` → local aggregation on each client |
| Shared Signal Chains | Each seat syncs `ShareSignal` → local merge with LWW |
| Multi-seat confidence | Count distinct `client_id`s for same signal_id in local sync_log |
| Team Briefings | Aggregate locally from all synced entries — no server computation |
| Cross-team correlation (Enterprise) | Org admin's client subscribes to multiple team streams |
| Audit log (Enterprise) | Each seat writes local audit entries + syncs metadata summaries |
| Webhooks (Enterprise) | Admin seat dispatches webhooks (single canonical source) |
| Org Dashboard (Enterprise) | Org admin aggregates from all teams locally |

The relay doesn't compute anything. It just routes encrypted blobs. All intelligence stays on the clients.
