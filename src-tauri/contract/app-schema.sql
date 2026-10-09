-- 4DA app-schema contract. GENERATED, do not edit by hand.
-- Regenerate: UPDATE_APP_SCHEMA_CONTRACT=1 cargo test --lib app_schema_contract
-- Consumer: github.com/4DA-Systems/4da-mcp-server (pnpm run contract)
-- schema_version: 125

CREATE TABLE accuracy_history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                period TEXT NOT NULL UNIQUE,
                total_scored INTEGER NOT NULL DEFAULT 0,
                total_relevant INTEGER NOT NULL DEFAULT 0,
                user_confirmed INTEGER DEFAULT 0,
                user_rejected INTEGER DEFAULT 0,
                accuracy_pct REAL DEFAULT 0.0,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );

CREATE TABLE accuracy_metrics (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            metric_date TEXT NOT NULL UNIQUE,
            precision_score REAL,
            recall_score REAL,
            engagement_rate REAL,
            items_shown INTEGER DEFAULT 0,
            items_clicked INTEGER DEFAULT 0,
            positive_feedback INTEGER DEFAULT 0,
            negative_feedback INTEGER DEFAULT 0,
            created_at TEXT DEFAULT (datetime('now'))
        );

CREATE TABLE active_topics (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            topic TEXT NOT NULL UNIQUE,
            weight REAL DEFAULT 0.5,
            confidence REAL DEFAULT 0.5,
            embedding BLOB,
            source TEXT NOT NULL,          -- 'file_content', 'git_commit', etc.
            last_seen TEXT DEFAULT (datetime('now')),
            decay_applied INTEGER DEFAULT 0,
            created_at TEXT DEFAULT (datetime('now'))
        );

CREATE TABLE advantage_score (
                                id INTEGER PRIMARY KEY AUTOINCREMENT,
                                period TEXT NOT NULL,
                                score REAL NOT NULL DEFAULT 0.0,
                                items_surfaced INTEGER NOT NULL DEFAULT 0,
                                avg_lead_time_hours REAL NOT NULL DEFAULT 0.0,
                                windows_opened INTEGER NOT NULL DEFAULT 0,
                                windows_acted INTEGER NOT NULL DEFAULT 0,
                                windows_expired INTEGER NOT NULL DEFAULT 0,
                                knowledge_gaps_closed INTEGER NOT NULL DEFAULT 0,
                                calibration_accuracy REAL NOT NULL DEFAULT 0.0,
                                computed_at TEXT NOT NULL DEFAULT (datetime('now'))
                            );

CREATE TABLE advisor_judgments (
                                source_item_id INTEGER NOT NULL,
                                identity_hash TEXT NOT NULL,
                                prompt_version TEXT NOT NULL,
                                raw_score REAL NOT NULL,
                                confidence REAL NOT NULL,
                                reasoning TEXT NOT NULL DEFAULT '',
                                judged_at TEXT NOT NULL DEFAULT (datetime('now')),
                                PRIMARY KEY (source_item_id, identity_hash, prompt_version),
                                FOREIGN KEY (source_item_id) REFERENCES source_items(id) ON DELETE CASCADE
                            );

CREATE TABLE agent_memory (
                            id INTEGER PRIMARY KEY AUTOINCREMENT,
                            session_id TEXT NOT NULL,
                            agent_type TEXT NOT NULL,
                            memory_type TEXT NOT NULL,
                            subject TEXT NOT NULL,
                            content TEXT NOT NULL,
                            context_tags TEXT DEFAULT '[]',
                            created_at TEXT NOT NULL DEFAULT (datetime('now')),
                            expires_at TEXT,
                            promoted_to_decision_id INTEGER,
                            FOREIGN KEY (promoted_to_decision_id) REFERENCES developer_decisions(id)
                        );

CREATE TABLE ai_usage (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                provider TEXT NOT NULL,
                model TEXT NOT NULL,
                task_type TEXT NOT NULL,
                tokens_in INTEGER DEFAULT 0,
                tokens_out INTEGER DEFAULT 0,
                estimated_cost_usd REAL DEFAULT 0.0,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );

CREATE TABLE alert_triage (
                                id INTEGER PRIMARY KEY AUTOINCREMENT,
                                item_id INTEGER NOT NULL,
                                advisory_id TEXT,
                                action TEXT NOT NULL CHECK(action IN ('investigating', 'fixed', 'not_applicable', 'accepted_risk', 'snoozed', 'acknowledged')),
                                reason TEXT,
                                resolved_at TEXT NOT NULL DEFAULT (datetime('now')),
                                expires_at TEXT,
                                UNIQUE(item_id)
                            );

CREATE TABLE anomalies (
            id INTEGER PRIMARY KEY,
            anomaly_type TEXT NOT NULL,
            topic TEXT,
            description TEXT NOT NULL,
            confidence REAL DEFAULT 0.5,
            severity TEXT DEFAULT 'medium',
            evidence TEXT DEFAULT '[]',
            detected_at TEXT DEFAULT (datetime('now')),
            resolved INTEGER DEFAULT 0
        );

CREATE TABLE app_meta (
                                key TEXT PRIMARY KEY,
                                value TEXT NOT NULL
                            );

CREATE TABLE audit_log (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                event_id TEXT NOT NULL UNIQUE,
                team_id TEXT NOT NULL,
                actor_id TEXT NOT NULL,
                actor_display_name TEXT NOT NULL,
                action TEXT NOT NULL,
                resource_type TEXT NOT NULL,
                resource_id TEXT,
                details TEXT,
                created_at TEXT DEFAULT (datetime('now'))
            );

CREATE TABLE autophagy_cycles (
                                id INTEGER PRIMARY KEY AUTOINCREMENT,
                                items_analyzed INTEGER NOT NULL DEFAULT 0,
                                items_pruned INTEGER NOT NULL DEFAULT 0,
                                calibrations_produced INTEGER NOT NULL DEFAULT 0,
                                topic_decay_rates_updated INTEGER NOT NULL DEFAULT 0,
                                source_autopsies_produced INTEGER NOT NULL DEFAULT 0,
                                anti_patterns_detected INTEGER NOT NULL DEFAULT 0,
                                db_size_before_bytes INTEGER NOT NULL DEFAULT 0,
                                db_size_after_bytes INTEGER NOT NULL DEFAULT 0,
                                duration_ms INTEGER NOT NULL DEFAULT 0,
                                created_at TEXT NOT NULL DEFAULT (datetime('now'))
                            );

CREATE TABLE blind_spot_dismissals (
                                id INTEGER PRIMARY KEY,
                                item_id TEXT NOT NULL UNIQUE,
                                reason TEXT NOT NULL,
                                dismissed_at DATETIME DEFAULT CURRENT_TIMESTAMP
                            );

CREATE TABLE bootstrap_paths (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            path TEXT NOT NULL UNIQUE,
            priority INTEGER DEFAULT 0,
            scanned INTEGER DEFAULT 0,
            last_scanned TEXT,
            created_at TEXT DEFAULT (datetime('now'))
        );

CREATE TABLE brief_rejections (
                                 id INTEGER PRIMARY KEY,
                                 briefing_id INTEGER,
                                 source_item_id INTEGER NOT NULL,
                                 reason TEXT NOT NULL,
                                 created_at TEXT NOT NULL DEFAULT (datetime('now'))
                             );

CREATE TABLE briefing_item_history (
                            id INTEGER PRIMARY KEY AUTOINCREMENT,
                            item_title TEXT NOT NULL,
                            source_type TEXT NOT NULL,
                            briefing_date TEXT NOT NULL,
                            created_at TEXT NOT NULL DEFAULT (datetime('now'))
                        , state_signature TEXT);

CREATE TABLE briefing_seals (
                                seal_id TEXT PRIMARY KEY,
                                seal_date TEXT NOT NULL,
                                seal_level INTEGER NOT NULL DEFAULT 0,
                                parent_seal_id TEXT,
                                summary_text TEXT NOT NULL,
                                item_count INTEGER NOT NULL,
                                top_topics TEXT NOT NULL DEFAULT '[]',
                                token_count INTEGER NOT NULL DEFAULT 0,
                                created_at INTEGER NOT NULL
                            );

CREATE TABLE briefings (
                            id INTEGER PRIMARY KEY AUTOINCREMENT,
                            content TEXT NOT NULL,
                            model TEXT,
                            item_count INTEGER NOT NULL DEFAULT 0,
                            tokens_used INTEGER,
                            latency_ms INTEGER,
                            created_at TEXT NOT NULL DEFAULT (datetime('now'))
                        );

CREATE TABLE calibration_samples (
                                id INTEGER PRIMARY KEY AUTOINCREMENT,
                                source_item_id INTEGER NOT NULL,
                                model_identity_hash TEXT NOT NULL,
                                task TEXT NOT NULL,
                                prompt_version TEXT NOT NULL,
                                raw_score REAL NOT NULL,
                                confidence REAL NOT NULL,
                                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                                processed_at TEXT
                             );

CREATE TABLE channel_provenance (
                                id INTEGER PRIMARY KEY AUTOINCREMENT,
                                render_id INTEGER NOT NULL,
                                claim_index INTEGER NOT NULL,
                                claim_text TEXT NOT NULL,
                                source_item_ids TEXT NOT NULL DEFAULT '[]',
                                source_titles TEXT NOT NULL DEFAULT '[]',
                                source_urls TEXT NOT NULL DEFAULT '[]',
                                FOREIGN KEY (render_id) REFERENCES channel_renders(id)
                            );

CREATE TABLE channel_renders (
                                id INTEGER PRIMARY KEY AUTOINCREMENT,
                                channel_id INTEGER NOT NULL,
                                version INTEGER NOT NULL,
                                content_markdown TEXT NOT NULL,
                                content_hash TEXT NOT NULL,
                                source_item_ids TEXT NOT NULL DEFAULT '[]',
                                model TEXT,
                                tokens_used INTEGER,
                                latency_ms INTEGER,
                                rendered_at TEXT NOT NULL DEFAULT (datetime('now')),
                                FOREIGN KEY (channel_id) REFERENCES channels(id),
                                UNIQUE(channel_id, version)
                            );

CREATE TABLE channel_source_matches (
                                channel_id INTEGER NOT NULL,
                                source_item_id INTEGER NOT NULL,
                                match_score REAL NOT NULL DEFAULT 0.0,
                                matched_at TEXT NOT NULL DEFAULT (datetime('now')),
                                PRIMARY KEY (channel_id, source_item_id),
                                FOREIGN KEY (channel_id) REFERENCES channels(id),
                                FOREIGN KEY (source_item_id) REFERENCES source_items(id)
                            );

CREATE TABLE channels (
                                id INTEGER PRIMARY KEY AUTOINCREMENT,
                                slug TEXT NOT NULL UNIQUE,
                                title TEXT NOT NULL,
                                description TEXT NOT NULL DEFAULT '',
                                topic_query TEXT NOT NULL DEFAULT '[]',
                                status TEXT NOT NULL DEFAULT 'active',
                                source_count INTEGER NOT NULL DEFAULT 0,
                                render_count INTEGER NOT NULL DEFAULT 0,
                                last_rendered_at TEXT,
                                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
                            );

CREATE TABLE coach_sessions (
                                id TEXT PRIMARY KEY,
                                session_type TEXT NOT NULL,
                                title TEXT NOT NULL DEFAULT 'New Session',
                                context_snapshot TEXT,
                                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
                            );

CREATE TABLE command_execution_log (
                                id INTEGER PRIMARY KEY AUTOINCREMENT,
                                module_id TEXT NOT NULL,
                                lesson_idx INTEGER NOT NULL,
                                command_id TEXT NOT NULL,
                                command_text TEXT NOT NULL,
                                success INTEGER NOT NULL,
                                exit_code INTEGER,
                                stdout TEXT,
                                stderr TEXT,
                                duration_ms INTEGER,
                                executed_at TEXT DEFAULT (datetime('now'))
                            );

CREATE TABLE command_history (
                            id INTEGER PRIMARY KEY AUTOINCREMENT,
                            command TEXT NOT NULL,
                            working_dir TEXT NOT NULL,
                            exit_code INTEGER,
                            success INTEGER NOT NULL DEFAULT 0,
                            output_preview TEXT,
                            created_at TEXT NOT NULL DEFAULT (datetime('now'))
                        );

CREATE TABLE commitment_contracts (
                                id INTEGER PRIMARY KEY AUTOINCREMENT,
                                decision_statement TEXT NOT NULL,
                                refutation_condition TEXT NOT NULL,
                                subject TEXT NOT NULL DEFAULT '',
                                status TEXT NOT NULL DEFAULT 'active',
                                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                                triggered_at TEXT,
                                trigger_item_id INTEGER,
                                FOREIGN KEY (trigger_item_id) REFERENCES source_items(id)
                             );

CREATE TABLE content_analyses (
                                id INTEGER PRIMARY KEY AUTOINCREMENT,
                                source_item_id INTEGER NOT NULL,
                                content_hash TEXT NOT NULL,
                                technical_depth INTEGER NOT NULL,
                                novelty INTEGER NOT NULL,
                                audience_level TEXT NOT NULL,
                                key_insight TEXT,
                                analyzed_at TEXT NOT NULL DEFAULT (datetime('now')),
                                UNIQUE(content_hash)
                            );

CREATE TABLE content_personalization_cache (
                                id INTEGER PRIMARY KEY AUTOINCREMENT,
                                module_id TEXT NOT NULL,
                                lesson_idx INTEGER NOT NULL,
                                block_type TEXT NOT NULL,
                                block_id TEXT NOT NULL,
                                content_json TEXT NOT NULL,
                                generation_path TEXT NOT NULL,
                                context_hash TEXT NOT NULL,
                                profile_hash TEXT NOT NULL,
                                llm_tokens_used INTEGER DEFAULT 0,
                                llm_cost_cents INTEGER DEFAULT 0,
                                generated_at TEXT DEFAULT (datetime('now')),
                                expires_at TEXT,
                                UNIQUE(module_id, lesson_idx, block_type, block_id, context_hash)
                            );

CREATE TABLE content_read_state (
                                id INTEGER PRIMARY KEY AUTOINCREMENT,
                                module_id TEXT NOT NULL,
                                lesson_idx INTEGER NOT NULL,
                                context_hash TEXT NOT NULL,
                                profile_snapshot TEXT NOT NULL,
                                read_at TEXT DEFAULT (datetime('now')),
                                UNIQUE(module_id, lesson_idx)
                            );

CREATE TABLE context_change_log (
                                 gen        INTEGER PRIMARY KEY AUTOINCREMENT,
                                 context_id INTEGER NOT NULL,
                                 deleted    INTEGER NOT NULL DEFAULT 0
                             );

CREATE TABLE context_chunks (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                source_file TEXT NOT NULL,
                content_hash TEXT NOT NULL UNIQUE,
                text TEXT NOT NULL,
                embedding BLOB NOT NULL,
                weight REAL NOT NULL DEFAULT 1.0,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
            , source_type TEXT DEFAULT 'text', page_number INTEGER, confidence REAL DEFAULT 1.0, extracted_at TEXT);

CREATE VIRTUAL TABLE context_vec USING vec0(
                                 id integer primary key,
                                 grounds integer partition key,
                                 embedding float[768]
                             );

CREATE TABLE decision_votes (
                decision_id TEXT NOT NULL,
                voter_id TEXT NOT NULL,
                stance TEXT NOT NULL,
                rationale TEXT,
                voted_at TEXT DEFAULT (datetime('now')),
                PRIMARY KEY (decision_id, voter_id)
            );

CREATE TABLE decision_windows (
                                id INTEGER PRIMARY KEY AUTOINCREMENT,
                                window_type TEXT NOT NULL,
                                title TEXT NOT NULL,
                                description TEXT NOT NULL DEFAULT '',
                                urgency REAL NOT NULL DEFAULT 0.5,
                                relevance REAL NOT NULL DEFAULT 0.5,
                                source_item_ids TEXT NOT NULL DEFAULT '[]',
                                signal_chain_id INTEGER,
                                dependency TEXT,
                                status TEXT NOT NULL DEFAULT 'open',
                                opened_at TEXT NOT NULL DEFAULT (datetime('now')),
                                expires_at TEXT,
                                acted_at TEXT,
                                closed_at TEXT,
                                outcome TEXT,
                                lead_time_hours REAL,
                                streets_engine TEXT
                            );

CREATE TABLE dependency_alerts (
                id INTEGER PRIMARY KEY,
                package_name TEXT NOT NULL,
                ecosystem TEXT NOT NULL,
                alert_type TEXT NOT NULL,
                severity TEXT NOT NULL,
                title TEXT NOT NULL,
                description TEXT,
                affected_versions TEXT,
                source_url TEXT,
                source_item_id INTEGER,
                detected_at TEXT NOT NULL DEFAULT (datetime('now')),
                resolved_at TEXT, project_path TEXT,
                FOREIGN KEY (source_item_id) REFERENCES source_items(id)
            );

CREATE TABLE "dependency_edges" (
                                 id INTEGER PRIMARY KEY,
                                 project_path TEXT NOT NULL,
                                 ecosystem TEXT NOT NULL,
                                 parent_package TEXT NOT NULL,
                                 parent_version TEXT,
                                 child_package TEXT NOT NULL,
                                 child_version TEXT,
                                 scope TEXT NOT NULL DEFAULT 'unknown',
                                 detected_at TEXT NOT NULL DEFAULT (datetime('now'))
                             );

CREATE TABLE dependency_instances (
                                 id INTEGER PRIMARY KEY,
                                 project_path TEXT NOT NULL,
                                 ecosystem TEXT NOT NULL,
                                 package_name TEXT NOT NULL,
                                 version TEXT NOT NULL,
                                 is_direct INTEGER NOT NULL DEFAULT 0,
                                 is_dev INTEGER NOT NULL DEFAULT 0,
                                 scope TEXT NOT NULL DEFAULT 'unknown',
                                 detected_at TEXT NOT NULL DEFAULT (datetime('now')),
                                 UNIQUE(project_path, ecosystem, package_name, version)
                             );

CREATE TABLE dependency_snapshots (
                                id INTEGER PRIMARY KEY,
                                project_path TEXT NOT NULL,
                                package_name TEXT NOT NULL,
                                ecosystem TEXT NOT NULL,
                                version TEXT,
                                is_direct INTEGER NOT NULL DEFAULT 1,
                                is_dev INTEGER NOT NULL DEFAULT 0,
                                source TEXT NOT NULL DEFAULT 'manifest',
                                scanned_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                                UNIQUE(project_path, package_name, ecosystem)
                            );

CREATE TABLE detected_projects (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            path TEXT NOT NULL UNIQUE,
            name TEXT NOT NULL,
            languages TEXT,                -- JSON array
            frameworks TEXT,               -- JSON array
            dependencies TEXT,             -- JSON array
            last_activity TEXT,
            detection_confidence REAL DEFAULT 0.5,
            -- 1 when the project directory is gitignored by the repository
            -- that encloses it: a scratch tree, a gauntlet, a throwaway
            -- fixture. It is still a real project with real dependencies —
            -- this only lets the surfaces SAY so, so the user can tell a
            -- scratch dir from a product. Never suppresses.
            scratch INTEGER NOT NULL DEFAULT 0,
            created_at TEXT DEFAULT (datetime('now')),
            updated_at TEXT DEFAULT (datetime('now'))
        );

CREATE TABLE detected_tech (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL UNIQUE,
            category TEXT NOT NULL,        -- 'language', 'framework', 'library', etc.
            confidence REAL DEFAULT 0.5,
            source TEXT NOT NULL,          -- 'manifest', 'file_extension', etc.
            evidence TEXT,                 -- Semicolon-separated evidence strings
            created_at TEXT DEFAULT (datetime('now')),
            updated_at TEXT DEFAULT (datetime('now'))
        , last_decay_at TEXT DEFAULT NULL);

CREATE TABLE developer_decisions (
                            id INTEGER PRIMARY KEY AUTOINCREMENT,
                            decision_type TEXT NOT NULL,
                            subject TEXT NOT NULL,
                            decision TEXT NOT NULL,
                            rationale TEXT,
                            alternatives_rejected TEXT DEFAULT '[]',
                            context_tags TEXT DEFAULT '[]',
                            confidence REAL NOT NULL DEFAULT 0.8,
                            status TEXT NOT NULL DEFAULT 'active',
                            superseded_by INTEGER,
                            created_at TEXT NOT NULL DEFAULT (datetime('now')),
                            updated_at TEXT NOT NULL DEFAULT (datetime('now')),
                            FOREIGN KEY (superseded_by) REFERENCES developer_decisions(id)
                        );

CREATE TABLE developer_timeline (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                period TEXT NOT NULL UNIQUE,
                tech_snapshot TEXT NOT NULL,
                interest_snapshot TEXT NOT NULL,
                decision_count INTEGER DEFAULT 0,
                feedback_count INTEGER DEFAULT 0,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );

CREATE TABLE digested_intelligence (
                                id INTEGER PRIMARY KEY AUTOINCREMENT,
                                digest_type TEXT NOT NULL,
                                subject TEXT NOT NULL,
                                data TEXT NOT NULL,
                                confidence REAL NOT NULL DEFAULT 0.5,
                                sample_size INTEGER NOT NULL DEFAULT 0,
                                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                                expires_at TEXT,
                                superseded_by INTEGER,
                                FOREIGN KEY (superseded_by) REFERENCES digested_intelligence(id)
                            );

CREATE TABLE document_chunks (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            document_id INTEGER NOT NULL,
            chunk_index INTEGER NOT NULL,
            content TEXT NOT NULL,
            word_count INTEGER DEFAULT 0,
            embedding BLOB,                    -- embedding for semantic search
            created_at TEXT DEFAULT (datetime('now')),
            FOREIGN KEY (document_id) REFERENCES indexed_documents(id) ON DELETE CASCADE
        );

CREATE TABLE domains (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                domain TEXT NOT NULL UNIQUE,
                created_at TEXT DEFAULT (datetime('now'))
            );

CREATE TABLE engine_runs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    trigger TEXT NOT NULL,
    started_at TEXT NOT NULL,
    completed_at TEXT NOT NULL,
    duration_ms INTEGER NOT NULL DEFAULT 0,
    sources_succeeded INTEGER NOT NULL DEFAULT 0,
    sources_failed INTEGER NOT NULL DEFAULT 0,
    sources_skipped INTEGER NOT NULL DEFAULT 0,
    new_items INTEGER NOT NULL DEFAULT 0,
    cached_touches INTEGER NOT NULL DEFAULT 0,
    items_scored INTEGER NOT NULL DEFAULT 0,
    relevant_count INTEGER NOT NULL DEFAULT 0,
    source_items_total INTEGER NOT NULL DEFAULT 0,
    max_item_created_at TEXT,
    content_fingerprint TEXT NOT NULL DEFAULT '',
    ok INTEGER NOT NULL DEFAULT 1,
    error TEXT,
    nonce TEXT,
    signature TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE error_telemetry (
                                id INTEGER PRIMARY KEY AUTOINCREMENT,
                                category TEXT NOT NULL,
                                message TEXT NOT NULL,
                                context TEXT,
                                count INTEGER NOT NULL DEFAULT 1,
                                first_seen TEXT NOT NULL DEFAULT (datetime('now')),
                                last_seen TEXT NOT NULL DEFAULT (datetime('now')),
                                UNIQUE(category, message)
                            );

CREATE TABLE exclusions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                topic TEXT NOT NULL UNIQUE,
                created_at TEXT DEFAULT (datetime('now'))
            );

CREATE TABLE explicit_interests (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                topic TEXT NOT NULL UNIQUE,
                weight REAL DEFAULT 1.0,
                embedding BLOB,
                source TEXT DEFAULT 'explicit',
                created_at TEXT DEFAULT (datetime('now'))
            );

CREATE TABLE extraction_jobs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                file_path TEXT NOT NULL,
                file_type TEXT NOT NULL,
                status TEXT NOT NULL CHECK(status IN ('pending', 'processing', 'completed', 'failed')),
                error TEXT,
                started_at TEXT,
                completed_at TEXT,
                extracted_chunks INTEGER DEFAULT 0,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );

CREATE TABLE facet_evidence (
                                id INTEGER PRIMARY KEY AUTOINCREMENT,
                                facet_id TEXT NOT NULL REFERENCES learned_facets(facet_id) ON DELETE CASCADE,
                                cue_family TEXT NOT NULL,
                                evidence_type TEXT NOT NULL,
                                confidence REAL NOT NULL,
                                observed_at INTEGER NOT NULL
                            );

CREATE TABLE feed_health (
                                feed_origin TEXT NOT NULL,
                                source_type TEXT NOT NULL,
                                consecutive_failures INTEGER NOT NULL DEFAULT 0,
                                total_successes INTEGER NOT NULL DEFAULT 0,
                                total_failures INTEGER NOT NULL DEFAULT 0,
                                last_success_at TEXT,
                                last_failure_at TEXT,
                                last_error TEXT,
                                circuit_open INTEGER NOT NULL DEFAULT 0,
                                circuit_opened_at TEXT,
                                circuit_reopen_count INTEGER NOT NULL DEFAULT 0,
                                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                                updated_at TEXT NOT NULL DEFAULT (datetime('now')),
                                PRIMARY KEY (feed_origin, source_type)
                            );

CREATE TABLE feedback (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                source_item_id INTEGER NOT NULL,
                relevant INTEGER NOT NULL,  -- 1 = relevant, 0 = not relevant
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                FOREIGN KEY (source_item_id) REFERENCES source_items(id)
            );

CREATE TABLE feedback_outbox (
                                id INTEGER PRIMARY KEY AUTOINCREMENT,
                                event_type TEXT NOT NULL,
                                signal_id TEXT,
                                alert_id TEXT,
                                source_type TEXT,
                                topic TEXT,
                                notes TEXT,
                                dismiss_reason TEXT,
                                dismiss_category TEXT,
                                queued_at INTEGER NOT NULL,
                                attempts INTEGER NOT NULL DEFAULT 0,
                                last_attempt_at INTEGER,
                                status TEXT NOT NULL DEFAULT 'pending'
                            );

CREATE TABLE file_signals (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            path TEXT NOT NULL,
            change_type TEXT NOT NULL,     -- 'created', 'modified', 'deleted'
            extracted_topics TEXT,         -- JSON array
            content_hash TEXT,
            timestamp TEXT DEFAULT (datetime('now')),
            processed INTEGER DEFAULT 0
        );

CREATE TABLE git_signals (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            repo_path TEXT NOT NULL,
            commit_hash TEXT,
            commit_message TEXT,
            extracted_topics TEXT,         -- JSON array
            files_changed TEXT,            -- JSON array
            timestamp TEXT DEFAULT (datetime('now'))
        );

CREATE TABLE glyph_audit (
                                id INTEGER PRIMARY KEY AUTOINCREMENT,
                                envelope_id TEXT NOT NULL,
                                agent TEXT NOT NULL,
                                logged_at TEXT NOT NULL,
                                summary TEXT NOT NULL,
                                compiled_nl TEXT NOT NULL,
                                header_glyphs TEXT NOT NULL,
                                verdict TEXT NOT NULL,
                                level TEXT NOT NULL,
                                payload_bytes INTEGER NOT NULL,
                                created_at TEXT NOT NULL DEFAULT (datetime('now'))
                             );

CREATE TABLE graph_layout_anchors (
                                 window_days INTEGER NOT NULL,
                                 cluster_key TEXT NOT NULL,
                                 x REAL NOT NULL,
                                 y REAL NOT NULL,
                                 member_ids TEXT NOT NULL,
                                 updated_at TEXT NOT NULL DEFAULT (datetime('now')),
                                 PRIMARY KEY (window_days, cluster_key)
                             );

CREATE TABLE identity_ledger (
                                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                                 entity_kind TEXT NOT NULL,
                                 entity_key  TEXT NOT NULL,
                                 change      TEXT NOT NULL,
                                 reason      TEXT,
                                 evidence    TEXT,
                                 at          TEXT NOT NULL DEFAULT (datetime('now'))
                             );

CREATE TABLE indexed_documents (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            file_path TEXT NOT NULL UNIQUE,
            file_name TEXT NOT NULL,
            file_type TEXT NOT NULL,           -- 'pdf', 'docx', 'xlsx', 'zip', etc.
            file_size INTEGER,
            content_hash TEXT,
            word_count INTEGER DEFAULT 0,
            page_count INTEGER DEFAULT 0,
            extraction_confidence REAL DEFAULT 0.0,
            extracted_topics TEXT,             -- JSON array
            last_modified TEXT,
            indexed_at TEXT DEFAULT (datetime('now')),
            updated_at TEXT DEFAULT (datetime('now'))
        );

CREATE TABLE intelligence_history (
                                id INTEGER PRIMARY KEY,
                                recorded_at TEXT NOT NULL DEFAULT (datetime('now')),
                                accuracy REAL NOT NULL,
                                topics_learned INTEGER NOT NULL,
                                items_analyzed INTEGER NOT NULL,
                                relevant_found INTEGER NOT NULL
                            );

CREATE TABLE interactions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            source_item_id INTEGER,             -- used by ContextEngine
            item_id INTEGER,                    -- used by ACE (nullable for ContextEngine compat)
            action TEXT,                        -- used by ContextEngine
            action_type TEXT,                   -- 'click', 'save', 'share', 'dismiss', etc.
            action_data TEXT,                   -- JSON with action-specific data (dwell_time, etc.)
            item_topics TEXT,                   -- JSON array
            item_source TEXT,                   -- 'hackernews', 'arxiv', etc.
            signal_strength REAL DEFAULT 0.5,
            timestamp TEXT DEFAULT (datetime('now'))
        );

CREATE TABLE item_context_cache (
                                 item_id    INTEGER PRIMARY KEY,
                                 generation INTEGER NOT NULL,
                                 builder    INTEGER NOT NULL
                             ) WITHOUT ROWID;

CREATE TABLE item_context_match (
                                 item_id    INTEGER NOT NULL,
                                 rank       INTEGER NOT NULL,
                                 context_id INTEGER NOT NULL,
                                 distance   REAL NOT NULL,
                                 PRIMARY KEY (item_id, rank)
                             ) WITHOUT ROWID;

CREATE TABLE item_necessity (
                            source_item_id INTEGER PRIMARY KEY REFERENCES source_items(id),
                            necessity_score REAL NOT NULL DEFAULT 0.0,
                            necessity_reason TEXT,
                            necessity_category TEXT,
                            necessity_urgency TEXT,
                            scored_at TEXT NOT NULL DEFAULT (datetime('now'))
                        );

CREATE TABLE kv_store (
                key TEXT PRIMARY KEY NOT NULL,
                value,
                updated_at TEXT DEFAULT (datetime('now'))
            );

CREATE TABLE learned_facets (
                                facet_id TEXT PRIMARY KEY,
                                class TEXT NOT NULL,
                                key TEXT NOT NULL,
                                value TEXT NOT NULL,
                                stability REAL NOT NULL DEFAULT 0.0,
                                state TEXT NOT NULL DEFAULT 'candidate',
                                user_state TEXT NOT NULL DEFAULT 'auto',
                                evidence_count INTEGER NOT NULL DEFAULT 0,
                                first_seen_at INTEGER NOT NULL,
                                last_seen_at INTEGER NOT NULL,
                                UNIQUE(class, key)
                            );

CREATE TABLE llm_judgments (
                                id INTEGER PRIMARY KEY AUTOINCREMENT,
                                source_item_id INTEGER NOT NULL,
                                relevance_score REAL NOT NULL,
                                explanation TEXT NOT NULL,
                                actions TEXT,
                                confidence REAL NOT NULL,
                                model TEXT NOT NULL,
                                prompt_version TEXT NOT NULL DEFAULT 'v1',
                                judged_at TEXT NOT NULL DEFAULT (datetime('now')),
                                UNIQUE(source_item_id, prompt_version)
                            );

CREATE TABLE migration_history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                from_version INTEGER NOT NULL,
                to_version INTEGER NOT NULL,
                executed_at TEXT NOT NULL DEFAULT (datetime('now')),
                duration_ms INTEGER NOT NULL DEFAULT 0,
                success INTEGER NOT NULL DEFAULT 0
            );

CREATE TABLE org_admins (
                org_id TEXT NOT NULL,
                member_id TEXT NOT NULL,
                role TEXT NOT NULL DEFAULT 'org_admin',
                PRIMARY KEY (org_id, member_id)
            );

CREATE TABLE org_teams (
                org_id TEXT NOT NULL,
                team_id TEXT NOT NULL,
                PRIMARY KEY (org_id, team_id)
            );

CREATE TABLE organizations (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                license_key_hash TEXT,
                settings TEXT,
                created_at TEXT DEFAULT (datetime('now'))
            );

CREATE TABLE osv_advisories (
                                id INTEGER PRIMARY KEY AUTOINCREMENT,
                                advisory_id TEXT NOT NULL,
                                summary TEXT NOT NULL,
                                details TEXT,
                                package_name TEXT NOT NULL,
                                ecosystem TEXT NOT NULL,
                                affected_ranges TEXT,
                                fixed_versions TEXT,
                                severity_type TEXT,
                                cvss_score REAL,
                                source_url TEXT,
                                published_at TEXT,
                                modified_at TEXT,
                                synced_at TEXT NOT NULL DEFAULT (datetime('now')),
                                aliases TEXT,
                                severity_label TEXT, withdrawn_at TEXT,
                                UNIQUE(advisory_id, package_name, ecosystem)
                            );

CREATE TABLE osv_sync_status (
                                ecosystem TEXT PRIMARY KEY,
                                last_synced_at TEXT NOT NULL,
                                advisory_count INTEGER NOT NULL DEFAULT 0,
                                error TEXT
                            );

CREATE TABLE playbook_progress (
                                module_id TEXT NOT NULL,
                                lesson_idx INTEGER NOT NULL,
                                completed_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                                PRIMARY KEY (module_id, lesson_idx)
                            );

CREATE TABLE precision_stats (
                                id INTEGER PRIMARY KEY AUTOINCREMENT,
                                period TEXT NOT NULL,
                                domain TEXT NOT NULL,
                                total_surfaced INTEGER DEFAULT 0,
                                true_positives INTEGER DEFAULT 0,
                                false_positives INTEGER DEFAULT 0,
                                false_negatives INTEGER DEFAULT 0,
                                acted_on INTEGER DEFAULT 0,
                                dismissed INTEGER DEFAULT 0,
                                precision REAL,
                                action_conversion_rate REAL,
                                avg_lead_time_hours REAL,
                                computed_at TEXT NOT NULL DEFAULT (datetime('now'))
                             );

CREATE TABLE preemption_wins (
                                id INTEGER PRIMARY KEY AUTOINCREMENT,
                                alert_id TEXT NOT NULL,
                                alert_title TEXT NOT NULL,
                                alerted_at TEXT NOT NULL,
                                incident_at TEXT,
                                lead_time_hours REAL,
                                affected_deps TEXT,
                                user_acted INTEGER DEFAULT 0,
                                verified INTEGER DEFAULT 0,
                                created_at TEXT NOT NULL DEFAULT (datetime('now'))
                             );

CREATE TABLE project_dependencies (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                project_path TEXT NOT NULL,
                manifest_type TEXT NOT NULL,
                package_name TEXT NOT NULL,
                version TEXT,
                is_dev INTEGER DEFAULT 0,
                language TEXT NOT NULL,
                last_scanned TEXT NOT NULL DEFAULT (datetime('now')), is_direct INTEGER DEFAULT 1, project_relevance REAL DEFAULT 1.0, target_cfg TEXT, platform_active INTEGER DEFAULT 1, detected_from TEXT NOT NULL DEFAULT 'unknown',
                UNIQUE(project_path, package_name)
            );

CREATE TABLE provenance (
                                id INTEGER PRIMARY KEY AUTOINCREMENT,
                                artifact_kind TEXT NOT NULL,
                                artifact_id TEXT NOT NULL,
                                model_identity_hash TEXT NOT NULL,
                                provider TEXT NOT NULL,
                                model TEXT NOT NULL,
                                prompt_version TEXT,
                                calibration_id TEXT,
                                task TEXT NOT NULL,
                                temperature REAL,
                                raw_response_hash TEXT,
                                shadow_peer_id INTEGER,
                                created_at TEXT NOT NULL DEFAULT (datetime('now'))
                             );

CREATE TABLE retention_policies (
                id TEXT PRIMARY KEY,
                team_id TEXT NOT NULL,
                resource_type TEXT NOT NULL,
                retention_days INTEGER NOT NULL,
                updated_at TEXT DEFAULT (datetime('now')),
                UNIQUE(team_id, resource_type)
            );

CREATE TABLE scheduler_state (
                                job_name TEXT PRIMARY KEY NOT NULL,
                                last_run_unix INTEGER NOT NULL DEFAULT 0,
                                last_duration_ms INTEGER,
                                run_count INTEGER NOT NULL DEFAULT 0,
                                last_outcome TEXT,
                                updated_at TEXT NOT NULL DEFAULT (datetime('now'))
                             );

CREATE TABLE schema_version (
                version INTEGER PRIMARY KEY
            );

CREATE TABLE scoring_churn (
                                id INTEGER PRIMARY KEY AUTOINCREMENT,
                                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                                path TEXT NOT NULL,
                                pipeline_version INTEGER NOT NULL,
                                items_written INTEGER NOT NULL,
                                rescored INTEGER NOT NULL,
                                moved_up_gt_010 INTEGER NOT NULL,
                                moved_down_gt_010 INTEGER NOT NULL,
                                max_up REAL NOT NULL,
                                max_down REAL NOT NULL,
                                mean_abs_delta REAL NOT NULL
                            , top_offenders TEXT, suppressed_writes INTEGER);

CREATE TABLE scoring_events (
                                id INTEGER PRIMARY KEY AUTOINCREMENT,
                                cycle_ts TEXT NOT NULL DEFAULT (datetime('now')),
                                total_scored INTEGER NOT NULL,
                                total_relevant INTEGER NOT NULL,
                                avg_score REAL NOT NULL,
                                max_score REAL NOT NULL,
                                gate_rejections INTEGER NOT NULL DEFAULT 0,
                                commodity_caps INTEGER NOT NULL DEFAULT 0,
                                enrichment_promotions INTEGER NOT NULL DEFAULT 0,
                                briefing_items INTEGER NOT NULL DEFAULT 0
                            );

CREATE TABLE scoring_explanations (
                                source_item_id INTEGER PRIMARY KEY,
                                pipeline_version INTEGER NOT NULL,
                                breakdown TEXT NOT NULL,
                                scored_at TEXT NOT NULL DEFAULT (datetime('now')),
                                FOREIGN KEY (source_item_id) REFERENCES source_items(id) ON DELETE CASCADE
                            );

CREATE TABLE scoring_stats (
                                id INTEGER PRIMARY KEY AUTOINCREMENT,
                                run_type TEXT NOT NULL,
                                total_scored INTEGER NOT NULL,
                                relevant_count INTEGER NOT NULL,
                                excluded_count INTEGER NOT NULL,
                                rejection_rate REAL NOT NULL,
                                created_at DATETIME DEFAULT CURRENT_TIMESTAMP
                            );

CREATE TABLE security_audit_log (
                                id INTEGER PRIMARY KEY,
                                timestamp TEXT NOT NULL DEFAULT (datetime('now')),
                                event_type TEXT NOT NULL,
                                details TEXT,
                                severity TEXT NOT NULL DEFAULT 'info'
                            );

CREATE TABLE selected_stacks (
                                id INTEGER PRIMARY KEY AUTOINCREMENT,
                                profile_id TEXT NOT NULL UNIQUE,
                                auto_detected INTEGER DEFAULT 0,
                                confidence REAL DEFAULT 1.0,
                                created_at TEXT DEFAULT (datetime('now'))
                            );

CREATE TABLE shared_resources (
                id TEXT PRIMARY KEY,
                team_id TEXT NOT NULL,
                resource_type TEXT NOT NULL,
                resource_data TEXT NOT NULL,
                shared_by TEXT NOT NULL,
                visibility TEXT DEFAULT 'team',
                visible_to TEXT,
                created_at TEXT DEFAULT (datetime('now')),
                expires_at TEXT
            );

CREATE TABLE snoozed_items (
                                 source_item_id INTEGER PRIMARY KEY,
                                 snooze_until TEXT NOT NULL,
                                 created_at TEXT NOT NULL DEFAULT (datetime('now'))
                             );

CREATE TABLE source_health (
                            source_type TEXT PRIMARY KEY,
                            status TEXT NOT NULL DEFAULT 'unknown',
                            last_success TEXT,
                            last_error TEXT,
                            error_count INTEGER NOT NULL DEFAULT 0,
                            consecutive_failures INTEGER NOT NULL DEFAULT 0,
                            items_fetched INTEGER NOT NULL DEFAULT 0,
                            response_time_ms INTEGER NOT NULL DEFAULT 0,
                            checked_at TEXT NOT NULL DEFAULT (datetime('now')),
                            circuit_reopen_count INTEGER NOT NULL DEFAULT 0,
                            retry_after_secs INTEGER
                        );

CREATE TABLE source_item_dependencies (
                                id INTEGER PRIMARY KEY,
                                source_item_id INTEGER NOT NULL,
                                package_name TEXT NOT NULL,
                                ecosystem TEXT,
                                match_type TEXT NOT NULL DEFAULT 'title_heuristic',
                                confidence REAL NOT NULL DEFAULT 0.5,
                                evidence_text TEXT,
                                source_url TEXT,
                                created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                                FOREIGN KEY (source_item_id) REFERENCES source_items(id) ON DELETE CASCADE
                            );

CREATE TABLE source_items (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                source_type TEXT NOT NULL,
                source_id TEXT NOT NULL,
                url TEXT,
                title TEXT NOT NULL,
                content TEXT NOT NULL DEFAULT '',
                content_hash TEXT NOT NULL,
                embedding BLOB NOT NULL,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                last_seen TEXT NOT NULL DEFAULT (datetime('now')), embedding_status TEXT DEFAULT 'complete', embed_text TEXT DEFAULT NULL, summary TEXT DEFAULT NULL, view_count INTEGER DEFAULT 0, detected_lang TEXT DEFAULT 'en', relevance_score REAL DEFAULT NULL, content_type TEXT DEFAULT NULL, cve_ids TEXT DEFAULT NULL, feed_origin TEXT, tags TEXT DEFAULT NULL, scored_pipeline_version INTEGER NOT NULL DEFAULT 0, signal_type TEXT, signal_priority TEXT, published_at TEXT DEFAULT NULL, feed_relevant INTEGER, feed_verdict_at TEXT, feed_verdict_version INTEGER, feed_verdict_source TEXT, feed_verdict_reason TEXT, feed_verdict_pending TEXT, rank_score REAL, rank_factors TEXT, rank_scored_at TEXT, content_updated_at TEXT, scored_at TEXT, first_curated_at TEXT,
                UNIQUE(source_type, source_id)
            );

CREATE VIRTUAL TABLE source_items_fts USING fts5(
                                title,
                                content,
                                content='source_items',
                                content_rowid='id',
                                tokenize='porter unicode61'
                            );

CREATE VIRTUAL TABLE source_vec USING vec0(
                                 embedding float[768]
                             );

CREATE TABLE sources (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                source_type TEXT NOT NULL UNIQUE,
                name TEXT NOT NULL,
                enabled INTEGER NOT NULL DEFAULT 1,
                config TEXT,  -- JSON config for the source
                last_fetch TEXT,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            );

CREATE TABLE sovereign_profile (
                                id INTEGER PRIMARY KEY AUTOINCREMENT,
                                category TEXT NOT NULL,
                                key TEXT NOT NULL,
                                value TEXT NOT NULL,
                                raw_output TEXT,
                                source_command TEXT,
                                source_lesson TEXT,
                                confidence REAL DEFAULT 1.0,
                                created_at TEXT DEFAULT (datetime('now')),
                                updated_at TEXT DEFAULT (datetime('now')),
                                UNIQUE(category, key)
                            );

CREATE TABLE sso_pending_auth (
                id TEXT PRIMARY KEY,
                state TEXT NOT NULL UNIQUE,
                nonce TEXT NOT NULL,
                provider_type TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                expires_at TEXT NOT NULL
            );

CREATE TABLE sun_alerts (
                                id INTEGER PRIMARY KEY AUTOINCREMENT,
                                sun_id TEXT NOT NULL,
                                alert_type TEXT NOT NULL,
                                message TEXT NOT NULL,
                                acknowledged INTEGER NOT NULL DEFAULT 0,
                                created_at TEXT DEFAULT (datetime('now'))
                            );

CREATE TABLE sun_runs (
                                id INTEGER PRIMARY KEY AUTOINCREMENT,
                                sun_id TEXT NOT NULL,
                                module_id TEXT NOT NULL,
                                success INTEGER NOT NULL,
                                result_message TEXT,
                                data_json TEXT,
                                duration_ms INTEGER,
                                created_at TEXT DEFAULT (datetime('now'))
                            );

CREATE TABLE team_alert_policies (
                team_id TEXT PRIMARY KEY,
                min_seats_to_alert INTEGER DEFAULT 2,
                aggregation_window_minutes INTEGER DEFAULT 60,
                notification_channels TEXT DEFAULT '["in_app"]',
                updated_at TEXT DEFAULT (datetime('now'))
            );

CREATE TABLE team_crypto (
                team_id             TEXT PRIMARY KEY,
                our_public_key      BLOB NOT NULL,
                our_private_key_enc BLOB NOT NULL,
                team_symmetric_key_enc BLOB,
                created_at          INTEGER NOT NULL DEFAULT (unixepoch())
            );

CREATE TABLE team_decisions (
                id TEXT PRIMARY KEY,
                team_id TEXT NOT NULL,
                title TEXT NOT NULL,
                decision_type TEXT NOT NULL,
                rationale TEXT NOT NULL,
                proposed_by TEXT NOT NULL,
                status TEXT DEFAULT 'proposed',
                created_at TEXT DEFAULT (datetime('now')),
                resolved_at TEXT
            );

CREATE TABLE team_members_cache (
                team_id      TEXT NOT NULL,
                client_id    TEXT NOT NULL,
                display_name TEXT NOT NULL,
                role         TEXT NOT NULL DEFAULT 'member',
                public_key   BLOB,
                last_seen    TEXT,
                PRIMARY KEY (team_id, client_id)
            );

CREATE TABLE team_signals (
                id TEXT PRIMARY KEY,
                team_id TEXT NOT NULL,
                signal_type TEXT NOT NULL,
                title TEXT NOT NULL,
                severity TEXT NOT NULL,
                tech_topics TEXT,
                detected_by_count INTEGER DEFAULT 1,
                first_detected TEXT DEFAULT (datetime('now')),
                last_detected TEXT DEFAULT (datetime('now')),
                resolved INTEGER DEFAULT 0,
                resolved_by TEXT,
                resolved_at TEXT,
                resolution_notes TEXT
            );

CREATE TABLE team_sync_log (
                relay_seq   INTEGER NOT NULL,
                team_id     TEXT NOT NULL,
                client_id   TEXT NOT NULL,
                encrypted   BLOB NOT NULL,
                received_at INTEGER NOT NULL DEFAULT (unixepoch()),
                applied     INTEGER NOT NULL DEFAULT 0,
                PRIMARY KEY (relay_seq, team_id)
            );

CREATE TABLE team_sync_queue (
                entry_id    TEXT PRIMARY KEY,
                team_id     TEXT NOT NULL,
                client_id   TEXT NOT NULL,
                operation   TEXT NOT NULL,
                hlc_ts      INTEGER NOT NULL,
                encrypted   BLOB,
                relay_seq   INTEGER,
                created_at  INTEGER NOT NULL DEFAULT (unixepoch()),
                acked_at    INTEGER
            );

CREATE TABLE team_sync_state (
                team_id         TEXT PRIMARY KEY,
                last_relay_seq  INTEGER NOT NULL DEFAULT 0,
                last_sync_at    INTEGER
            );

CREATE TABLE tech_stack (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                technology TEXT NOT NULL UNIQUE,
                created_at TEXT DEFAULT (datetime('now'))
            );

CREATE TABLE temporal_events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                event_type TEXT NOT NULL,
                subject TEXT NOT NULL,
                data JSON NOT NULL,
                embedding BLOB,
                source_item_id INTEGER,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                expires_at TEXT
            );

CREATE TABLE toolkit_http_history (
                                id INTEGER PRIMARY KEY AUTOINCREMENT,
                                method TEXT NOT NULL,
                                url TEXT NOT NULL,
                                status INTEGER NOT NULL,
                                duration_ms INTEGER NOT NULL DEFAULT 0,
                                created_at TEXT NOT NULL DEFAULT (datetime('now'))
                            );

CREATE TABLE topic_hotness (
                                topic_key TEXT PRIMARY KEY,
                                mention_count INTEGER NOT NULL DEFAULT 0,
                                distinct_sources INTEGER NOT NULL DEFAULT 0,
                                last_seen_at INTEGER NOT NULL,
                                query_hits INTEGER NOT NULL DEFAULT 0,
                                hotness_score REAL NOT NULL DEFAULT 0.0,
                                materialized INTEGER NOT NULL DEFAULT 0,
                                first_seen_at INTEGER NOT NULL
                            );

CREATE TABLE topic_hotness_sources (
                                day_source_key TEXT PRIMARY KEY,
                                topic_key TEXT NOT NULL,
                                source_type TEXT NOT NULL,
                                seen_at INTEGER NOT NULL
                            );

CREATE VIRTUAL TABLE topic_vec USING vec0(
            embedding float[768]
        );

CREATE TABLE translation_cache (
                                id INTEGER PRIMARY KEY AUTOINCREMENT,
                                content_hash TEXT NOT NULL,
                                source_lang TEXT NOT NULL DEFAULT 'en',
                                target_lang TEXT NOT NULL,
                                source_text TEXT NOT NULL,
                                translated_text TEXT NOT NULL,
                                provider TEXT NOT NULL,
                                model_version TEXT,
                                created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
                                last_used_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
                                use_count INTEGER NOT NULL DEFAULT 1
                            );

CREATE TABLE trust_events (
                                id INTEGER PRIMARY KEY AUTOINCREMENT,
                                event_type TEXT NOT NULL,
                                signal_id TEXT,
                                alert_id TEXT,
                                source_type TEXT,
                                topic TEXT,
                                lead_time_hours REAL,
                                user_action TEXT,
                                outcome TEXT,
                                confidence_at_surface REAL,
                                notes TEXT,
                                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                                resolved_at TEXT
                             );

CREATE TABLE user_dependencies (
                id INTEGER PRIMARY KEY,
                project_path TEXT NOT NULL,
                package_name TEXT NOT NULL,
                version TEXT,
                ecosystem TEXT NOT NULL,
                is_dev INTEGER DEFAULT 0,
                is_direct INTEGER DEFAULT 1,
                detected_at TEXT NOT NULL DEFAULT (datetime('now')),
                last_seen_at TEXT NOT NULL DEFAULT (datetime('now')), license TEXT, detected_from TEXT NOT NULL DEFAULT 'unknown', platform_active INTEGER NOT NULL DEFAULT 1, target_cfg TEXT,
                UNIQUE(project_path, package_name, ecosystem)
            );

CREATE TABLE user_events (
                                id INTEGER PRIMARY KEY AUTOINCREMENT,
                                event_type TEXT NOT NULL,
                                view_id TEXT,
                                metadata TEXT,
                                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                                session_id TEXT
                            );

CREATE TABLE user_identity (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                role TEXT,
                created_at TEXT DEFAULT (datetime('now')),
                updated_at TEXT DEFAULT (datetime('now'))
            , experience_level TEXT);

CREATE TABLE validated_signals (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            signal_type TEXT NOT NULL,
            signal_data TEXT NOT NULL,     -- JSON
            confidence REAL NOT NULL,
            evidence_sources TEXT,         -- JSON array
            contradictions TEXT,           -- JSON array
            freshness REAL,
            timestamp TEXT DEFAULT (datetime('now'))
        );

CREATE TABLE void_positions (
                item_id INTEGER NOT NULL,
                item_type TEXT NOT NULL,
                x REAL NOT NULL,
                y REAL NOT NULL,
                z REAL NOT NULL,
                projection_version INTEGER NOT NULL,
                PRIMARY KEY (item_id, item_type)
            );

CREATE TABLE waitlist_signups (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                tier TEXT NOT NULL,
                email TEXT NOT NULL,
                name TEXT,
                team_size TEXT,
                company TEXT,
                role TEXT,
                source TEXT DEFAULT 'in-app',
                signed_up_at TEXT NOT NULL DEFAULT (datetime('now')),
                UNIQUE(email, tier)
            );

CREATE TABLE watched_directories (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                path TEXT NOT NULL UNIQUE,
                enabled INTEGER DEFAULT 1,
                last_indexed TEXT,
                chunk_count INTEGER DEFAULT 0,
                created_at TEXT DEFAULT (datetime('now'))
            );

CREATE TABLE webhook_deliveries (
                id TEXT PRIMARY KEY,
                webhook_id TEXT NOT NULL,
                event_type TEXT NOT NULL,
                payload TEXT NOT NULL,
                status TEXT DEFAULT 'pending',
                http_status INTEGER,
                attempt_count INTEGER DEFAULT 0,
                next_retry_at TEXT,
                created_at TEXT DEFAULT (datetime('now')),
                delivered_at TEXT,
                FOREIGN KEY (webhook_id) REFERENCES webhooks(id)
            );

CREATE TABLE webhooks (
                id TEXT PRIMARY KEY,
                team_id TEXT NOT NULL,
                name TEXT NOT NULL,
                url TEXT NOT NULL,
                events TEXT NOT NULL,
                secret TEXT NOT NULL,
                active INTEGER DEFAULT 1,
                failure_count INTEGER DEFAULT 0,
                last_fired_at TEXT,
                last_status_code INTEGER,
                created_at TEXT DEFAULT (datetime('now')),
                created_by TEXT
            );

CREATE INDEX idx_accuracy_metrics_date ON accuracy_metrics(metric_date);

CREATE INDEX idx_active_topics_last_seen ON active_topics(last_seen);

CREATE INDEX idx_active_topics_topic ON active_topics(topic);

CREATE INDEX idx_advantage_period
                                ON advantage_score(period, computed_at);

CREATE INDEX idx_advisor_judgments_judged_at
                                ON advisor_judgments(judged_at);

CREATE INDEX idx_agent_memory_expires ON agent_memory(expires_at);

CREATE INDEX idx_agent_memory_session ON agent_memory(session_id);

CREATE INDEX idx_agent_memory_subject ON agent_memory(subject);

CREATE INDEX idx_agent_memory_type ON agent_memory(memory_type);

CREATE INDEX idx_ai_usage_date ON ai_usage(created_at);

CREATE INDEX idx_ai_usage_provider ON ai_usage(provider, model);

CREATE INDEX idx_ai_usage_task ON ai_usage(task_type);

CREATE INDEX idx_alert_triage_expires ON alert_triage(expires_at) WHERE expires_at IS NOT NULL;

CREATE INDEX idx_alert_triage_item ON alert_triage(item_id);

CREATE INDEX idx_anomalies_resolved ON anomalies(resolved);

CREATE INDEX idx_anomalies_type ON anomalies(anomaly_type);

CREATE INDEX idx_audit_action
                ON audit_log(action);

CREATE INDEX idx_audit_actor
                ON audit_log(actor_id, created_at DESC);

CREATE INDEX idx_audit_team_time
                ON audit_log(team_id, created_at DESC);

CREATE INDEX idx_brief_rejections_item
                                 ON brief_rejections (source_item_id);

CREATE INDEX idx_briefing_history_date ON briefing_item_history(briefing_date);

CREATE INDEX idx_briefing_history_signature
                                     ON briefing_item_history(source_type, state_signature, briefing_date);

CREATE INDEX idx_bsd_item ON blind_spot_dismissals(item_id);

CREATE INDEX idx_cal_samples_created
                               ON calibration_samples(created_at);

CREATE INDEX idx_cal_samples_item
                               ON calibration_samples(source_item_id, created_at);

CREATE INDEX idx_cal_samples_unfit
                               ON calibration_samples(model_identity_hash, task, processed_at);

CREATE INDEX idx_channel_provenance_render
                                ON channel_provenance(render_id);

CREATE INDEX idx_channel_renders_channel
                                ON channel_renders(channel_id);

CREATE INDEX idx_channel_renders_channel_version ON channel_renders(channel_id, version);

CREATE INDEX idx_channel_source_matches_channel
                                ON channel_source_matches(channel_id);

CREATE INDEX idx_channels_slug ON channels(slug);

CREATE INDEX idx_channels_status ON channels(status);

CREATE INDEX idx_cmd_history_created ON command_history(created_at);

CREATE INDEX idx_cmd_log_module
                                ON command_execution_log(module_id);

CREATE INDEX idx_coach_sessions_type
                                ON coach_sessions(session_type);

CREATE INDEX idx_coach_sessions_updated
                                ON coach_sessions(updated_at);

CREATE INDEX idx_content_analyses_hash ON content_analyses(content_hash);

CREATE INDEX idx_content_analyses_item ON content_analyses(source_item_id);

CREATE INDEX idx_context_hash ON context_chunks(content_hash);

CREATE INDEX idx_context_source ON context_chunks(source_file);

CREATE INDEX idx_contracts_status
                               ON commitment_contracts(status);

CREATE INDEX idx_contracts_subject
                               ON commitment_contracts(subject);

CREATE INDEX idx_decisions_status ON developer_decisions(status);

CREATE INDEX idx_decisions_subject ON developer_decisions(subject);

CREATE INDEX idx_decisions_type ON developer_decisions(decision_type);

CREATE INDEX idx_deliveries_pending
                ON webhook_deliveries(status, next_retry_at)
                WHERE status IN ('pending', 'failed');

CREATE INDEX idx_dep_alerts_package ON dependency_alerts(package_name, ecosystem);

CREATE INDEX idx_dep_alerts_severity ON dependency_alerts(severity);

CREATE INDEX idx_dep_edges_child
                                 ON dependency_edges (project_path, child_package);

CREATE INDEX idx_dep_edges_parent
                                 ON dependency_edges (project_path, parent_package);

CREATE UNIQUE INDEX idx_dep_edges_unique
                                 ON dependency_edges (project_path, ecosystem, parent_package,
                                                      COALESCE(parent_version, ''), child_package,
                                                      COALESCE(child_version, ''));

CREATE INDEX idx_dep_instances_pkg
                                 ON dependency_instances (ecosystem, package_name);

CREATE INDEX idx_dep_instances_project
                                 ON dependency_instances (project_path, ecosystem);

CREATE INDEX idx_deps_package ON project_dependencies(package_name);

CREATE INDEX idx_deps_project ON project_dependencies(project_path);

CREATE INDEX idx_deps_relevance ON project_dependencies(project_relevance);

CREATE INDEX idx_detected_tech_confidence ON detected_tech(confidence);

CREATE INDEX idx_detected_tech_name ON detected_tech(name);

CREATE INDEX idx_digest_created
                                ON digested_intelligence(created_at);

CREATE INDEX idx_digest_superseded ON digested_intelligence(superseded_by);

CREATE INDEX idx_digest_type_subject
                                ON digested_intelligence(digest_type, subject);

CREATE INDEX idx_document_chunks_doc ON document_chunks(document_id);

CREATE INDEX idx_ds_package ON dependency_snapshots(package_name);

CREATE INDEX idx_ds_project ON dependency_snapshots(project_path);

CREATE INDEX idx_ds_scanned ON dependency_snapshots(scanned_at);

CREATE INDEX idx_dw_dependency ON decision_windows(dependency);

CREATE INDEX idx_dw_status ON decision_windows(status);

CREATE INDEX idx_dw_type ON decision_windows(window_type);

CREATE INDEX idx_engine_runs_completed ON engine_runs(completed_at);

CREATE INDEX idx_error_telemetry_category ON error_telemetry(category);

CREATE INDEX idx_error_telemetry_last_seen ON error_telemetry(last_seen);

CREATE INDEX idx_evidence_facet ON facet_evidence(facet_id);

CREATE INDEX idx_evidence_observed ON facet_evidence(observed_at DESC);

CREATE INDEX idx_exclusions_topic ON exclusions(topic);

CREATE INDEX idx_extraction_jobs_file_path ON extraction_jobs(file_path);

CREATE INDEX idx_extraction_jobs_status ON extraction_jobs(status);

CREATE INDEX idx_facets_class_state ON learned_facets(class, state);

CREATE INDEX idx_facets_stability ON learned_facets(stability DESC);

CREATE INDEX idx_feed_health_circuit ON feed_health(circuit_open) WHERE circuit_open = 1;

CREATE INDEX idx_feed_health_source_type ON feed_health(source_type);

CREATE INDEX idx_feedback_created ON feedback(created_at);

CREATE INDEX idx_feedback_created_at ON feedback(created_at);

CREATE INDEX idx_feedback_item ON feedback(source_item_id);

CREATE INDEX idx_feedback_item_relevant ON feedback(source_item_id, relevant);

CREATE UNIQUE INDEX idx_feedback_outbox_dedup
                                ON feedback_outbox(event_type, COALESCE(signal_id,''), COALESCE(alert_id,''), COALESCE(source_type,''), COALESCE(topic,''), status);

CREATE INDEX idx_feedback_outbox_status
                                ON feedback_outbox(status, attempts);

CREATE INDEX idx_feedback_relevant ON feedback(relevant);

CREATE INDEX idx_file_signals_processed ON file_signals(processed);

CREATE INDEX idx_file_signals_timestamp ON file_signals(timestamp);

CREATE INDEX idx_git_signals_repo ON git_signals(repo_path);

CREATE INDEX idx_git_signals_timestamp ON git_signals(timestamp);

CREATE INDEX idx_glyph_audit_agent     ON glyph_audit(agent);

CREATE INDEX idx_glyph_audit_envelope  ON glyph_audit(envelope_id);

CREATE INDEX idx_glyph_audit_level     ON glyph_audit(level);

CREATE INDEX idx_glyph_audit_logged_at ON glyph_audit(logged_at);

CREATE INDEX idx_hotness_materialized ON topic_hotness(materialized, hotness_score DESC);

CREATE INDEX idx_hotness_score ON topic_hotness(hotness_score DESC);

CREATE INDEX idx_hotness_sources_topic ON topic_hotness_sources(topic_key);

CREATE INDEX idx_http_history_created
                                ON toolkit_http_history(created_at);

CREATE INDEX idx_identity_ledger_at
                                 ON identity_ledger(at);

CREATE INDEX idx_identity_ledger_entity
                                 ON identity_ledger(entity_kind, entity_key, at);

CREATE INDEX idx_indexed_documents_indexed ON indexed_documents(indexed_at);

CREATE INDEX idx_indexed_documents_path ON indexed_documents(file_path);

CREATE INDEX idx_indexed_documents_type ON indexed_documents(file_type);

CREATE INDEX idx_intelligence_history_recorded
                                ON intelligence_history(recorded_at);

CREATE INDEX idx_interactions_action ON interactions(action);

CREATE INDEX idx_interactions_item ON interactions(source_item_id);

CREATE INDEX idx_interactions_item_action ON interactions(item_id, action_type);

CREATE INDEX idx_interactions_item_id ON interactions(item_id);

CREATE INDEX idx_interactions_source ON interactions(item_source);

CREATE INDEX idx_interactions_timestamp ON interactions(timestamp);

CREATE INDEX idx_interests_topic ON explicit_interests(topic);

CREATE INDEX idx_item_context_cache_gen
                                 ON item_context_cache(generation);

CREATE INDEX idx_llm_judgments_item
                                ON llm_judgments(source_item_id);

CREATE INDEX idx_llm_judgments_relevance
                                ON llm_judgments(relevance_score DESC);

CREATE INDEX idx_match_type ON source_item_dependencies(match_type);

CREATE INDEX idx_necessity_score ON item_necessity(necessity_score);

CREATE INDEX idx_osv_advisories_advisory
                                ON osv_advisories(advisory_id);

CREATE INDEX idx_osv_advisories_cvss
                                ON osv_advisories(cvss_score DESC);

CREATE INDEX idx_osv_advisories_package
                                ON osv_advisories(package_name, ecosystem);

CREATE INDEX idx_personalization_cache_lookup
                                ON content_personalization_cache(module_id, lesson_idx, context_hash);

CREATE INDEX idx_pkg_eco ON source_item_dependencies(package_name, ecosystem);

CREATE INDEX idx_provenance_artifact
                               ON provenance(artifact_kind, artifact_id);

CREATE INDEX idx_provenance_created_at
                               ON provenance(created_at);

CREATE INDEX idx_provenance_model
                               ON provenance(model_identity_hash);

CREATE INDEX idx_provenance_task
                               ON provenance(task);

CREATE INDEX idx_scoring_churn_created
                                ON scoring_churn(created_at);

CREATE INDEX idx_scoring_events_ts ON scoring_events(cycle_ts);

CREATE INDEX idx_seals_level_date ON briefing_seals(seal_level, seal_date DESC);

CREATE INDEX idx_seals_parent ON briefing_seals(parent_seal_id);

CREATE INDEX idx_security_audit_event
                                ON security_audit_log(event_type);

CREATE INDEX idx_security_audit_timestamp
                                ON security_audit_log(timestamp);

CREATE INDEX idx_selected_stacks_profile
                                ON selected_stacks(profile_id);

CREATE INDEX idx_shared_expires
                ON shared_resources(expires_at) WHERE expires_at IS NOT NULL;

CREATE INDEX idx_shared_team_type
                ON shared_resources(team_id, resource_type);

CREATE INDEX idx_si_feed_relevant
                                 ON source_items(feed_relevant, created_at)
                                 WHERE feed_relevant IS NOT NULL;

CREATE INDEX idx_si_feed_verdict_version
                                 ON source_items(feed_verdict_version, feed_verdict_source)
                                 WHERE feed_relevant = 1;

CREATE INDEX idx_sid_pkg ON source_item_dependencies(source_item_id, package_name);

CREATE UNIQUE INDEX idx_sid_pkg_eco ON source_item_dependencies(source_item_id, package_name);

CREATE INDEX idx_snoozed_until
                                 ON snoozed_items(snooze_until);

CREATE INDEX idx_source_content_type ON source_items(content_type);

CREATE INDEX idx_source_embedding_status ON source_items(embedding_status);

CREATE INDEX idx_source_feed_origin ON source_items(feed_origin);

CREATE INDEX idx_source_hash ON source_items(content_hash);

CREATE INDEX idx_source_items_created ON source_items(created_at);

CREATE INDEX idx_source_items_created_at ON source_items(created_at);

CREATE INDEX idx_source_items_detected_lang ON source_items(detected_lang);

CREATE INDEX idx_source_items_effective_published
                                     ON source_items(COALESCE(published_at, created_at));

CREATE INDEX idx_source_items_embedding_status ON source_items(embedding_status);

CREATE INDEX idx_source_items_relevance_score ON source_items(relevance_score);

CREATE INDEX idx_source_items_scored_version
                                 ON source_items(scored_pipeline_version);

CREATE INDEX idx_source_seen ON source_items(last_seen);

CREATE INDEX idx_source_type ON source_items(source_type);

CREATE INDEX idx_source_type_created ON source_items(source_type, created_at);

CREATE INDEX idx_sovereign_category
                                ON sovereign_profile(category);

CREATE INDEX idx_sso_pending_expires ON sso_pending_auth(expires_at);

CREATE INDEX idx_sso_pending_state ON sso_pending_auth(state);

CREATE INDEX idx_sun_alerts_ack
                                ON sun_alerts(acknowledged);

CREATE INDEX idx_sun_runs_created
                                ON sun_runs(created_at);

CREATE INDEX idx_sun_runs_id
                                ON sun_runs(sun_id);

CREATE INDEX idx_team_decisions_team
                ON team_decisions(team_id, status);

CREATE INDEX idx_team_signals_team
                ON team_signals(team_id, resolved);

CREATE INDEX idx_temporal_expires ON temporal_events(expires_at);

CREATE INDEX idx_temporal_subject ON temporal_events(subject);

CREATE INDEX idx_temporal_type_time ON temporal_events(event_type, created_at);

CREATE INDEX idx_timeline_period ON developer_timeline(period);

CREATE INDEX idx_translation_cache_expiry
                                ON translation_cache(last_used_at);

CREATE UNIQUE INDEX idx_translation_cache_lookup
                                ON translation_cache(content_hash, target_lang);

CREATE INDEX idx_tsl_unapplied
                ON team_sync_log(applied) WHERE applied = 0;

CREATE INDEX idx_tsq_pending
                ON team_sync_queue(acked_at) WHERE acked_at IS NULL;

CREATE INDEX idx_user_deps_ecosystem ON user_dependencies(ecosystem);

CREATE INDEX idx_user_deps_package ON user_dependencies(package_name);

CREATE INDEX idx_user_events_created ON user_events(created_at);

CREATE INDEX idx_user_events_type ON user_events(event_type);

CREATE INDEX idx_validated_signals_timestamp ON validated_signals(timestamp);

CREATE INDEX idx_validated_signals_type ON validated_signals(signal_type);

CREATE INDEX idx_void_positions_version
                ON void_positions(projection_version);

CREATE INDEX idx_waitlist_tier ON waitlist_signups(tier);

CREATE INDEX idx_webhooks_team
                ON webhooks(team_id, active);

CREATE TRIGGER context_chunks_change_ad
                                 AFTER DELETE ON context_chunks BEGIN
                                 INSERT INTO context_change_log(context_id, deleted)
                                     VALUES (old.id, 1);
                             END;

CREATE TRIGGER context_chunks_change_ai
                                 AFTER INSERT ON context_chunks BEGIN
                                 INSERT INTO context_change_log(context_id, deleted)
                                     VALUES (new.id, 0);
                             END;

CREATE TRIGGER context_chunks_change_au
                                 AFTER UPDATE ON context_chunks BEGIN
                                 INSERT INTO context_change_log(context_id, deleted)
                                     VALUES (new.id, 0);
                             END;

CREATE TRIGGER item_context_cache_gone
                                 AFTER DELETE ON source_items
                             BEGIN
                                 DELETE FROM item_context_cache WHERE item_id = old.id;
                                 DELETE FROM item_context_match WHERE item_id = old.id;
                             END;

CREATE TRIGGER item_context_cache_reembed
                                 AFTER UPDATE OF embedding ON source_items
                                 WHEN old.embedding IS NOT new.embedding
                             BEGIN
                                 DELETE FROM item_context_cache WHERE item_id = new.id;
                                 DELETE FROM item_context_match WHERE item_id = new.id;
                             END;

CREATE TRIGGER trg_channel_renders_cascade_delete
                             AFTER DELETE ON channel_renders
                             BEGIN
                                 DELETE FROM channel_provenance WHERE render_id = OLD.id;
                             END;

CREATE TRIGGER trg_channels_cascade_delete
                             AFTER DELETE ON channels
                             BEGIN
                                 DELETE FROM channel_renders WHERE channel_id = OLD.id;
                                 DELETE FROM channel_source_matches WHERE channel_id = OLD.id;
                             END;

CREATE TRIGGER trg_source_items_cascade_delete
                             AFTER DELETE ON source_items
                             BEGIN
                                 DELETE FROM feedback WHERE source_item_id = OLD.id;
                                 DELETE FROM item_necessity WHERE source_item_id = OLD.id;
                                 DELETE FROM channel_source_matches WHERE source_item_id = OLD.id;
                                 DELETE FROM content_analyses WHERE source_item_id = OLD.id;
                             END;

CREATE TRIGGER trg_source_items_delete_dep_alerts
                             AFTER DELETE ON source_items
                             BEGIN
                                 DELETE FROM dependency_alerts WHERE source_item_id = OLD.id;
                             END;

CREATE TRIGGER trg_source_items_fts_delete
             AFTER DELETE ON source_items
             BEGIN
                 INSERT INTO source_items_fts(source_items_fts, rowid, title, content)
                 VALUES ('delete', OLD.id, COALESCE(OLD.title, ''), COALESCE(OLD.content, ''));
             END;

CREATE TRIGGER trg_source_items_fts_insert
             AFTER INSERT ON source_items
             BEGIN
                 INSERT INTO source_items_fts(rowid, title, content)
                 VALUES (NEW.id, COALESCE(NEW.title, ''), COALESCE(NEW.content, ''));
             END;

CREATE TRIGGER trg_source_items_fts_update
             AFTER UPDATE OF title, content ON source_items
             WHEN OLD.title IS NOT NEW.title OR OLD.content IS NOT NEW.content
             BEGIN
                 INSERT INTO source_items_fts(source_items_fts, rowid, title, content)
                 VALUES ('delete', OLD.id, COALESCE(OLD.title, ''), COALESCE(OLD.content, ''));
                 INSERT INTO source_items_fts(rowid, title, content)
                 VALUES (NEW.id, COALESCE(NEW.title, ''), COALESCE(NEW.content, ''));
             END;

CREATE TRIGGER trg_webhooks_delete_deliveries
                             AFTER DELETE ON webhooks
                             BEGIN
                                 DELETE FROM webhook_deliveries WHERE webhook_id = OLD.id;
                             END;

CREATE VIEW current_dependencies AS
                            SELECT ds.* FROM dependency_snapshots ds
                            INNER JOIN (
                                SELECT project_path, package_name, ecosystem, MAX(scanned_at) as latest
                                FROM dependency_snapshots
                                GROUP BY project_path, package_name, ecosystem
                            ) latest ON ds.project_path = latest.project_path
                                AND ds.package_name = latest.package_name
                                AND ds.ecosystem = latest.ecosystem
                                AND ds.scanned_at = latest.latest;

INSERT INTO schema_version (version) VALUES (125);
