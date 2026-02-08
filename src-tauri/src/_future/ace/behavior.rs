//! Behavior Learner - Implicit preference detection
//!
//! Learns user preferences from actions without explicit input:
//! - Click tracking with dwell time
//! - Save/share detection
//! - Dismissal and ignore patterns
//! - Topic affinity computation
//! - Anti-topic detection

#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Behavior tracking configuration
#[derive(Debug, Clone)]
pub struct BehaviorConfig {
    /// Minimum exposures before computing affinity
    pub min_exposures: u32,
    /// Rejection count to trigger anti-topic
    pub anti_topic_threshold: u32,
    /// Decay half-life in days
    pub decay_half_life_days: f32,
    /// Maximum history entries per topic
    pub max_history: usize,
}

impl Default for BehaviorConfig {
    fn default() -> Self {
        Self {
            min_exposures: 5,
            anti_topic_threshold: 5,
            decay_half_life_days: 30.0,
            max_history: 1000,
        }
    }
}

/// A behavior signal from user interaction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BehaviorSignal {
    pub item_id: i64,
    pub action: BehaviorAction,
    pub timestamp: String,
    /// Topics associated with the item
    pub item_topics: Vec<String>,
    /// Source of the item (e.g., "hackernews", "arxiv")
    pub item_source: String,
    /// Computed signal strength
    pub signal_strength: f32,
}

/// Types of user behavior we track
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BehaviorAction {
    /// User clicked on item
    Click {
        /// How long they stayed (seconds)
        dwell_time_seconds: u64,
    },
    /// User saved/bookmarked item
    Save,
    /// User shared item
    Share,
    /// User explicitly dismissed item
    Dismiss,
    /// User marked item as irrelevant
    MarkIrrelevant,
    /// User scrolled past (weak signal)
    Scroll {
        /// How long item was visible
        visible_seconds: f32,
    },
    /// User ignored item (showed but no action)
    Ignore,
}

impl BehaviorAction {
    /// Compute the signal strength for this action
    pub fn compute_strength(&self) -> f32 {
        match self {
            BehaviorAction::Click { dwell_time_seconds } => {
                let base = 0.5;
                // Bonus for dwell time, capped at +0.5 for 60+ seconds
                let dwell_bonus = (*dwell_time_seconds as f32 / 60.0).min(0.5);
                base + dwell_bonus
            }
            BehaviorAction::Save => 1.0,
            BehaviorAction::Share => 1.0,
            BehaviorAction::Dismiss => -0.8,
            BehaviorAction::MarkIrrelevant => -1.0,
            BehaviorAction::Scroll { visible_seconds } => {
                // Weak positive signal for visibility
                0.1 * visible_seconds.min(3.0)
            }
            BehaviorAction::Ignore => -0.1,
        }
    }

    /// Check if this action indicates strong engagement
    pub fn is_strong(&self) -> bool {
        matches!(
            self,
            BehaviorAction::Save | BehaviorAction::Share | BehaviorAction::Dismiss | BehaviorAction::MarkIrrelevant
        )
    }

    /// Check if this is a positive action
    pub fn is_positive(&self) -> bool {
        self.compute_strength() > 0.0
    }

    /// Check if this is a negative action
    pub fn is_negative(&self) -> bool {
        self.compute_strength() < 0.0
    }
}

/// Topic affinity computed from behavior
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopicAffinity {
    pub topic: String,
    pub embedding: Option<Vec<f32>>,
    /// Number of positive interactions
    pub positive_signals: u32,
    /// Number of negative interactions
    pub negative_signals: u32,
    /// Total times this topic was shown
    pub total_exposures: u32,
    /// Computed affinity score (-1.0 to 1.0)
    pub affinity_score: f32,
    /// Confidence based on exposure count
    pub confidence: f32,
    /// Last interaction timestamp
    pub last_interaction: String,
    /// Whether temporal decay has been applied
    pub decay_applied: bool,
}

impl TopicAffinity {
    /// Create a new topic affinity entry
    pub fn new(topic: String) -> Self {
        Self {
            topic,
            embedding: None,
            positive_signals: 0,
            negative_signals: 0,
            total_exposures: 0,
            affinity_score: 0.0,
            confidence: 0.0,
            last_interaction: chrono::Utc::now().to_rfc3339(),
            decay_applied: false,
        }
    }

    /// Record an interaction
    pub fn record_interaction(&mut self, signal_strength: f32) {
        self.total_exposures += 1;
        if signal_strength > 0.0 {
            self.positive_signals += 1;
        } else if signal_strength < 0.0 {
            self.negative_signals += 1;
        }
        self.last_interaction = chrono::Utc::now().to_rfc3339();
        self.decay_applied = false;
        self.recompute_score();
    }

    /// Recompute affinity score based on signals
    pub fn recompute_score(&mut self) {
        if self.total_exposures < 5 {
            self.affinity_score = 0.0;
            self.confidence = 0.0;
            return;
        }

        // Raw score: (positive - negative) / total
        let raw_score = (self.positive_signals as f32 - self.negative_signals as f32)
            / self.total_exposures as f32;

        // Confidence factor: scales with exposure count, maxes at 20
        let confidence_factor = (self.total_exposures as f32 / 20.0).min(1.0);

        // Apply temporal decay
        let decay_factor = self.compute_temporal_decay();

        self.affinity_score = raw_score * confidence_factor * decay_factor;
        self.confidence = confidence_factor * decay_factor;
    }

    /// Compute temporal decay based on last interaction
    fn compute_temporal_decay(&self) -> f32 {
        // Parse last_interaction timestamp
        if let Ok(last) = chrono::DateTime::parse_from_rfc3339(&self.last_interaction) {
            let days_since =
                (chrono::Utc::now() - last.with_timezone(&chrono::Utc)).num_hours() as f32 / 24.0;
            // Half-life of 30 days
            0.5_f32.powf(days_since / 30.0)
        } else {
            1.0 // No decay if we can't parse
        }
    }

    /// Apply decay and mark as processed
    pub fn apply_decay(&mut self) {
        if !self.decay_applied {
            self.recompute_score();
            self.decay_applied = true;
        }
    }

    /// Is this topic showing negative affinity?
    pub fn is_negative(&self) -> bool {
        self.affinity_score < -0.3 && self.confidence > 0.5
    }

    /// Is this a strong interest?
    pub fn is_strong_interest(&self) -> bool {
        self.affinity_score > 0.5 && self.confidence > 0.5
    }
}

/// A topic that the user consistently rejects
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AntiTopic {
    pub topic: String,
    /// Number of times rejected
    pub rejection_count: u32,
    /// Confidence in anti-topic status
    pub confidence: f32,
    /// Was this auto-detected (vs user-confirmed)?
    pub auto_detected: bool,
    /// Has user explicitly confirmed?
    pub user_confirmed: bool,
    /// Timestamp of first rejection
    pub first_rejection: String,
    /// Timestamp of last rejection
    pub last_rejection: String,
}

impl AntiTopic {
    /// Create a new anti-topic entry
    pub fn new(topic: String) -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        Self {
            topic,
            rejection_count: 1,
            confidence: 0.2,
            auto_detected: true,
            user_confirmed: false,
            first_rejection: now.clone(),
            last_rejection: now,
        }
    }

    /// Record another rejection
    pub fn record_rejection(&mut self) {
        self.rejection_count += 1;
        self.last_rejection = chrono::Utc::now().to_rfc3339();
        self.update_confidence();
    }

    /// Update confidence based on rejection count
    fn update_confidence(&mut self) {
        // Confidence grows with rejections, caps at 0.9 for auto-detected
        self.confidence = if self.user_confirmed {
            1.0
        } else {
            (self.rejection_count as f32 / 10.0).min(0.9)
        };
    }

    /// Mark as user-confirmed
    pub fn confirm(&mut self) {
        self.user_confirmed = true;
        self.confidence = 1.0;
    }
}

/// Source preferences learned from behavior
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourcePreference {
    pub source: String,
    /// Preference score (-1.0 to 1.0)
    pub score: f32,
    /// Number of interactions
    pub interactions: u32,
    /// Last interaction
    pub last_interaction: String,
}

impl SourcePreference {
    pub fn new(source: String) -> Self {
        Self {
            source,
            score: 0.0,
            interactions: 0,
            last_interaction: chrono::Utc::now().to_rfc3339(),
        }
    }

    pub fn record_interaction(&mut self, signal_strength: f32) {
        self.interactions += 1;
        // Moving average with decay toward new signal
        let alpha = 0.1; // Learning rate
        self.score = self.score * (1.0 - alpha) + signal_strength * alpha;
        self.last_interaction = chrono::Utc::now().to_rfc3339();
    }
}

/// Activity patterns by time of day
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ActivityPatterns {
    /// Engagement by hour (0-23)
    pub hourly_engagement: [f32; 24],
    /// Engagement by day of week (0=Sunday, 6=Saturday)
    pub daily_engagement: [f32; 7],
    /// Total interactions tracked
    pub total_tracked: u32,
}

impl ActivityPatterns {
    /// Record an interaction at the current time
    pub fn record_interaction(&mut self, signal_strength: f32) {
        let now = chrono::Utc::now();
        let hour = now.format("%H").to_string().parse::<usize>().unwrap_or(0);
        let day = now.format("%w").to_string().parse::<usize>().unwrap_or(0);

        // Update with exponential moving average
        let alpha = 0.05;
        self.hourly_engagement[hour] =
            self.hourly_engagement[hour] * (1.0 - alpha) + signal_strength * alpha;
        self.daily_engagement[day] =
            self.daily_engagement[day] * (1.0 - alpha) + signal_strength * alpha;
        self.total_tracked += 1;
    }

    /// Get the best hours for engagement
    pub fn best_hours(&self) -> Vec<usize> {
        let mut hours: Vec<(usize, f32)> = self
            .hourly_engagement
            .iter()
            .enumerate()
            .map(|(i, &v)| (i, v))
            .collect();
        hours.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        hours.iter().take(3).map(|(h, _)| *h).collect()
    }

    /// Get the best days for engagement
    pub fn best_days(&self) -> Vec<usize> {
        let mut days: Vec<(usize, f32)> = self
            .daily_engagement
            .iter()
            .enumerate()
            .map(|(i, &v)| (i, v))
            .collect();
        days.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        days.iter().take(3).map(|(d, _)| *d).collect()
    }
}

/// The complete learned behavior model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearnedBehavior {
    /// Topic affinities from behavior
    pub topic_affinities: HashMap<String, TopicAffinity>,
    /// Anti-topics (consistently rejected)
    pub anti_topics: Vec<AntiTopic>,
    /// Source preferences
    pub source_preferences: HashMap<String, SourcePreference>,
    /// Time-of-day patterns
    pub activity_patterns: ActivityPatterns,
    /// Confidence in learning (based on data volume)
    pub learning_confidence: f32,
    /// Last model update
    pub last_updated: String,
}

impl LearnedBehavior {
    pub fn new() -> Self {
        Self {
            topic_affinities: HashMap::new(),
            anti_topics: Vec::new(),
            source_preferences: HashMap::new(),
            activity_patterns: ActivityPatterns::default(),
            learning_confidence: 0.0,
            last_updated: chrono::Utc::now().to_rfc3339(),
        }
    }

    /// Process a behavior signal
    pub fn process_signal(&mut self, signal: &BehaviorSignal) {
        // Update topic affinities
        for topic in &signal.item_topics {
            let affinity = self
                .topic_affinities
                .entry(topic.clone())
                .or_insert_with(|| TopicAffinity::new(topic.clone()));
            affinity.record_interaction(signal.signal_strength);

            // Check for anti-topic
            if signal.signal_strength < -0.5 {
                self.record_rejection(topic);
            }
        }

        // Update source preference
        let source_pref = self
            .source_preferences
            .entry(signal.item_source.clone())
            .or_insert_with(|| SourcePreference::new(signal.item_source.clone()));
        source_pref.record_interaction(signal.signal_strength);

        // Update activity patterns
        self.activity_patterns
            .record_interaction(signal.signal_strength);

        // Update learning confidence
        self.update_learning_confidence();
        self.last_updated = chrono::Utc::now().to_rfc3339();
    }

    /// Record a rejection for potential anti-topic
    fn record_rejection(&mut self, topic: &str) {
        if let Some(anti) = self.anti_topics.iter_mut().find(|a| a.topic == topic) {
            anti.record_rejection();
        } else {
            self.anti_topics.push(AntiTopic::new(topic.to_string()));
        }
    }

    /// Update overall learning confidence
    fn update_learning_confidence(&mut self) {
        let total_interactions: u32 = self
            .topic_affinities
            .values()
            .map(|a| a.total_exposures)
            .sum();

        // Confidence grows with interactions, caps at 0.95
        self.learning_confidence = (total_interactions as f32 / 100.0).min(0.95);
    }

    /// Get topics with strong positive affinity
    pub fn get_interests(&self) -> Vec<&TopicAffinity> {
        self.topic_affinities
            .values()
            .filter(|a| a.is_strong_interest())
            .collect()
    }

    /// Get confirmed anti-topics
    pub fn get_anti_topics(&self, threshold: u32) -> Vec<&AntiTopic> {
        self.anti_topics
            .iter()
            .filter(|a| a.rejection_count >= threshold)
            .collect()
    }

    /// Apply decay to all affinities
    pub fn apply_decay(&mut self) {
        for affinity in self.topic_affinities.values_mut() {
            affinity.apply_decay();
        }
        self.last_updated = chrono::Utc::now().to_rfc3339();
    }

    /// Get behavior modifier for an item based on its topics
    pub fn get_behavior_modifier(&self, topics: &[String], source: &str) -> f32 {
        let mut modifier = 0.0;
        let mut topic_count = 0;

        // Topic affinity contribution
        for topic in topics {
            if let Some(affinity) = self.topic_affinities.get(topic) {
                modifier += affinity.affinity_score * affinity.confidence;
                topic_count += 1;
            }
        }

        // Average topic contribution
        if topic_count > 0 {
            modifier /= topic_count as f32;
        }

        // Source preference contribution (weighted less)
        if let Some(source_pref) = self.source_preferences.get(source) {
            modifier += source_pref.score * 0.3;
        }

        // Clamp to reasonable range
        modifier.clamp(-1.0, 1.0)
    }
}

impl Default for LearnedBehavior {
    fn default() -> Self {
        Self::new()
    }
}

/// Behavior learner that persists to database
pub struct BehaviorLearner {
    config: BehaviorConfig,
    /// In-memory cache of learned behavior
    cached_behavior: Option<LearnedBehavior>,
}

impl BehaviorLearner {
    pub fn new(config: BehaviorConfig) -> Self {
        Self {
            config,
            cached_behavior: None,
        }
    }

    /// Create a behavior signal from a user action
    pub fn create_signal(
        item_id: i64,
        action: BehaviorAction,
        item_topics: Vec<String>,
        item_source: String,
    ) -> BehaviorSignal {
        let signal_strength = action.compute_strength();
        BehaviorSignal {
            item_id,
            action,
            timestamp: chrono::Utc::now().to_rfc3339(),
            item_topics,
            item_source,
            signal_strength,
        }
    }

    /// Should this topic be flagged as anti-topic?
    pub fn should_be_anti_topic(&self, rejection_count: u32) -> bool {
        rejection_count >= self.config.anti_topic_threshold
    }

    /// Get the minimum exposures required for affinity computation
    pub fn min_exposures(&self) -> u32 {
        self.config.min_exposures
    }

    /// Get cached behavior (if available)
    pub fn get_cached(&self) -> Option<&LearnedBehavior> {
        self.cached_behavior.as_ref()
    }

    /// Update cached behavior
    pub fn set_cached(&mut self, behavior: LearnedBehavior) {
        self.cached_behavior = Some(behavior);
    }
}

impl Default for BehaviorLearner {
    fn default() -> Self {
        Self::new(BehaviorConfig::default())
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_behavior_action_strength() {
        // Click with dwell time (30 seconds = 0.5 base + 0.5 bonus = 1.0)
        let click = BehaviorAction::Click {
            dwell_time_seconds: 30,
        };
        assert!(click.compute_strength() > 0.5);
        assert!(click.compute_strength() <= 1.0);

        // Save has maximum positive strength
        assert_eq!(BehaviorAction::Save.compute_strength(), 1.0);

        // Dismiss is strongly negative
        assert_eq!(BehaviorAction::Dismiss.compute_strength(), -0.8);

        // Mark irrelevant is most negative
        assert_eq!(BehaviorAction::MarkIrrelevant.compute_strength(), -1.0);

        // Ignore is weakly negative
        assert_eq!(BehaviorAction::Ignore.compute_strength(), -0.1);
    }

    #[test]
    fn test_topic_affinity_computation() {
        let mut affinity = TopicAffinity::new("rust".to_string());

        // Not enough data yet
        affinity.recompute_score();
        assert_eq!(affinity.affinity_score, 0.0);

        // Add positive signals
        for _ in 0..4 {
            affinity.record_interaction(1.0);
        }

        // Still not enough (need 5)
        assert_eq!(affinity.affinity_score, 0.0);

        // Add one more
        affinity.record_interaction(1.0);

        // Now we have enough data
        assert!(affinity.affinity_score > 0.0);
        assert!(affinity.confidence > 0.0);
    }

    #[test]
    fn test_anti_topic_detection() {
        let mut behavior = LearnedBehavior::new();

        // Dismiss an item with "crypto" topic 5 times
        for _ in 0..5 {
            let signal = BehaviorLearner::create_signal(
                1,
                BehaviorAction::Dismiss,
                vec!["crypto".to_string()],
                "hackernews".to_string(),
            );
            behavior.process_signal(&signal);
        }

        // Should be flagged as anti-topic
        let anti_topics = behavior.get_anti_topics(5);
        assert_eq!(anti_topics.len(), 1);
        assert_eq!(anti_topics[0].topic, "crypto");
    }

    #[test]
    fn test_source_preference() {
        let mut behavior = LearnedBehavior::new();

        // Click on several HN items
        for _ in 0..5 {
            let signal = BehaviorLearner::create_signal(
                1,
                BehaviorAction::Click {
                    dwell_time_seconds: 60,
                },
                vec!["programming".to_string()],
                "hackernews".to_string(),
            );
            behavior.process_signal(&signal);
        }

        // Should have positive source preference
        let pref = behavior.source_preferences.get("hackernews");
        assert!(pref.is_some());
        assert!(pref.unwrap().score > 0.0);
    }

    #[test]
    fn test_behavior_modifier() {
        let mut behavior = LearnedBehavior::new();

        // Build up affinity for "rust"
        for _ in 0..10 {
            let signal = BehaviorLearner::create_signal(
                1,
                BehaviorAction::Save,
                vec!["rust".to_string()],
                "hackernews".to_string(),
            );
            behavior.process_signal(&signal);
        }

        // Build up negative affinity for "crypto"
        for _ in 0..10 {
            let signal = BehaviorLearner::create_signal(
                2,
                BehaviorAction::Dismiss,
                vec!["crypto".to_string()],
                "hackernews".to_string(),
            );
            behavior.process_signal(&signal);
        }

        // Check modifiers
        let rust_modifier = behavior.get_behavior_modifier(&["rust".to_string()], "hackernews");
        let crypto_modifier = behavior.get_behavior_modifier(&["crypto".to_string()], "hackernews");

        assert!(rust_modifier > 0.0, "Rust should have positive modifier");
        assert!(
            crypto_modifier < 0.0,
            "Crypto should have negative modifier"
        );
    }

    #[test]
    fn test_activity_patterns() {
        let mut patterns = ActivityPatterns::default();

        // Record some interactions
        for _ in 0..10 {
            patterns.record_interaction(0.8);
        }

        assert!(patterns.total_tracked == 10);
    }
}
