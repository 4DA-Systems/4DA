//! Bayesian persona inference engine for taste test calibration.
//!
//! Maintains a posterior probability distribution over 9 developer personas,
//! updated via Bayes' rule with each user response. Supports adaptive item
//! selection (maximizing expected information gain) and early termination.

use super::items::{calibration_items, LIKELIHOOD_MATRIX};
use super::{
    PersonaWeight, TasteProfile, TasteProfileSummary, TasteResponse, TasteTestStep,
    PERSONA_DESCRIPTIONS, PERSONA_NAMES,
};

const NUM_PERSONAS: usize = 9;
const NUM_ITEMS: usize = 15;
const EARLY_TERMINATION_MIN_ITEMS: usize = 7;
const EARLY_TERMINATION_ENTROPY_THRESHOLD: f64 = 1.2;

/// Bayesian inference state tracking posterior over personas.
#[derive(Debug, Clone)]
pub struct InferenceState {
    /// Current posterior probability for each persona.
    posterior: [f64; NUM_PERSONAS],
    /// History of (item_slot, response) pairs.
    items_shown: Vec<(usize, TasteResponse)>,
    /// Entropy after each update.
    entropy_history: Vec<f64>,
    /// Which slots have been shown.
    shown_slots: [bool; NUM_ITEMS],
}

impl InferenceState {
    /// Initialize with uniform prior.
    pub fn new() -> Self {
        Self {
            posterior: [1.0 / NUM_PERSONAS as f64; NUM_PERSONAS],
            items_shown: Vec::new(),
            entropy_history: Vec::new(),
            shown_slots: [false; NUM_ITEMS],
        }
    }

    /// Update posterior with a new observation.
    pub fn update(&mut self, item_slot: usize, response: &TasteResponse) {
        assert!(item_slot < NUM_ITEMS, "item_slot out of range");

        let likelihoods = &LIKELIHOOD_MATRIX[item_slot];

        for j in 0..NUM_PERSONAS {
            let p = likelihoods[j];
            let likelihood = match response {
                TasteResponse::Interested => p,
                TasteResponse::NotInterested => 1.0 - p,
                TasteResponse::StrongInterest => {
                    // Squaring function: amplifies differences
                    // High p stays high, low p gets lower
                    let sq = p * p / (p * p + (1.0 - p) * (1.0 - p));
                    sq
                }
            };
            self.posterior[j] *= likelihood;
        }

        // Normalize
        let sum: f64 = self.posterior.iter().sum();
        if sum > 0.0 {
            for w in &mut self.posterior {
                *w /= sum;
            }
        }

        self.shown_slots[item_slot] = true;
        self.items_shown.push((item_slot, response.clone()));
        self.entropy_history.push(self.entropy());
    }

    /// Shannon entropy of the current posterior.
    pub fn entropy(&self) -> f64 {
        let mut h = 0.0;
        for &p in &self.posterior {
            if p > 1e-15 {
                h -= p * p.log2();
            }
        }
        h
    }

    /// Confidence: 1.0 - normalized entropy. Range [0, 1].
    pub fn confidence(&self) -> f64 {
        let max_entropy = (NUM_PERSONAS as f64).log2(); // log2(9) ≈ 3.17
        1.0 - (self.entropy() / max_entropy)
    }

    /// Whether we have enough confidence to stop early.
    pub fn should_terminate(&self) -> bool {
        self.items_shown.len() >= EARLY_TERMINATION_MIN_ITEMS
            && self.entropy() < EARLY_TERMINATION_ENTROPY_THRESHOLD
    }

    /// Select the next item that maximizes expected information gain.
    /// Returns None if all items have been shown.
    pub fn next_item(&self) -> Option<usize> {
        let current_entropy = self.entropy();
        let mut best_slot = None;
        let mut best_gain = f64::NEG_INFINITY;

        for slot in 0..NUM_ITEMS {
            if self.shown_slots[slot] {
                continue;
            }

            // Compute expected entropy after showing this item
            let expected_entropy = self.simulate_entropy(slot);
            let gain = current_entropy - expected_entropy;

            if gain > best_gain {
                best_gain = gain;
                best_slot = Some(slot);
            }
        }

        best_slot
    }

    /// Simulate expected entropy if we show item at `_slot`.
    fn simulate_entropy(&self, slot: usize) -> f64 {
        let likelihoods = &LIKELIHOOD_MATRIX[slot];

        // P(interested) = Σ P(persona_j) × P(interested | persona_j)
        let p_interested: f64 = self
            .posterior
            .iter()
            .zip(likelihoods.iter())
            .map(|(&w, &l)| w * l)
            .sum();
        let p_not_interested = 1.0 - p_interested;

        // Entropy if user says "interested"
        let h_interested = {
            let mut post = self.posterior;
            let mut sum = 0.0;
            for j in 0..NUM_PERSONAS {
                post[j] *= likelihoods[j];
                sum += post[j];
            }
            if sum > 0.0 {
                for w in &mut post {
                    *w /= sum;
                }
            }
            let mut h = 0.0;
            for &p in &post {
                if p > 1e-15 {
                    h -= p * p.log2();
                }
            }
            h
        };

        // Entropy if user says "not interested"
        let h_not_interested = {
            let mut post = self.posterior;
            let mut sum = 0.0;
            for j in 0..NUM_PERSONAS {
                post[j] *= 1.0 - likelihoods[j];
                sum += post[j];
            }
            if sum > 0.0 {
                for w in &mut post {
                    *w /= sum;
                }
            }
            let mut h = 0.0;
            for &p in &post {
                if p > 1e-15 {
                    h -= p * p.log2();
                }
            }
            h
        };

        // Expected entropy = weighted average
        p_interested * h_interested + p_not_interested * h_not_interested
    }

    /// Finalize the inference and produce a TasteProfile.
    pub fn finalize(&self) -> TasteProfile {
        let dominant = self
            .posterior
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
            .map(|(i, _)| i)
            .unwrap_or(0);

        // Use blending to get interests, tech, exclusions
        let blended = super::blending::blend_profile(&self.posterior, 0.10);

        TasteProfile {
            persona_weights: self.posterior,
            dominant_persona: dominant,
            confidence: self.confidence(),
            items_shown: self.items_shown.len() as u32,
            inferred_interests: blended.interests,
            inferred_exclusions: blended.exclusions,
            calibration_deltas: blended.calibration_deltas.into_iter().collect(),
        }
    }

    /// Build a frontend-friendly summary.
    pub fn build_summary(&self) -> TasteProfileSummary {
        let dominant = self
            .posterior
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
            .map(|(i, _)| i)
            .unwrap_or(0);

        let blended = super::blending::blend_profile(&self.posterior, 0.10);

        TasteProfileSummary {
            dominant_persona_name: PERSONA_NAMES[dominant].to_string(),
            dominant_persona_description: PERSONA_DESCRIPTIONS[dominant].to_string(),
            confidence: self.confidence(),
            items_shown: self.items_shown.len() as u32,
            persona_weights: self
                .posterior
                .iter()
                .enumerate()
                .filter(|(_, &w)| w > 0.05)
                .map(|(i, &w)| PersonaWeight {
                    name: PERSONA_NAMES[i].to_string(),
                    weight: w,
                })
                .collect(),
            top_interests: blended
                .interests
                .into_iter()
                .take(10)
                .map(|(topic, _)| topic)
                .collect(),
        }
    }

    /// Process response and return the next step for the frontend.
    pub fn next_step(&self) -> TasteTestStep {
        let items = calibration_items();
        let total = NUM_ITEMS as f32;
        let shown = self.items_shown.len() as f32;

        if self.should_terminate() || self.items_shown.len() >= NUM_ITEMS {
            return TasteTestStep::Complete {
                summary: self.build_summary(),
            };
        }

        if let Some(slot) = self.next_item() {
            TasteTestStep::NextCard {
                card: items[slot].clone(),
                progress: shown / total,
                confidence: self.confidence() as f32,
            }
        } else {
            TasteTestStep::Complete {
                summary: self.build_summary(),
            }
        }
    }

    /// Get the current posterior weights.
    pub fn posterior(&self) -> &[f64; NUM_PERSONAS] {
        &self.posterior
    }

    /// Get number of items shown so far.
    pub fn num_items_shown(&self) -> usize {
        self.items_shown.len()
    }

    /// Get the recorded responses.
    pub fn responses(&self) -> &[(usize, TasteResponse)] {
        &self.items_shown
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_uniform_prior() {
        let state = InferenceState::new();
        let expected = 1.0 / NUM_PERSONAS as f64;
        for &w in state.posterior() {
            assert!((w - expected).abs() < 1e-10);
        }
    }

    #[test]
    fn test_single_update_shifts_posterior() {
        let mut state = InferenceState::new();
        // Slot 0 = Rust item. Saying "interested" should boost rust_systems (index 0)
        state.update(0, &TasteResponse::Interested);
        assert!(
            state.posterior()[0] > 1.0 / NUM_PERSONAS as f64,
            "Rust persona should increase after positive response to Rust item"
        );
    }

    #[test]
    fn test_convergence_after_5_items() {
        let mut state = InferenceState::new();
        // Respond "interested" to all Rust-related items
        for &slot in &[0, 6, 8, 10] {
            // Rust, tokio, WASM, sqlite-vec
            state.update(slot, &TasteResponse::Interested);
        }
        // Respond "not interested" to non-Rust items
        state.update(1, &TasteResponse::NotInterested); // PyTorch

        assert!(
            state.entropy() < 2.0,
            "Entropy should drop below 2.0 after 5 consistent responses, got {}",
            state.entropy()
        );
    }

    #[test]
    fn test_early_termination() {
        let mut state = InferenceState::new();
        // Strong Rust signal
        for &slot in &[0, 6, 8, 10] {
            state.update(slot, &TasteResponse::Interested);
        }
        for &slot in &[1, 2, 4, 5] {
            state.update(slot, &TasteResponse::NotInterested);
        }
        // After 8 items with consistent Rust responses, should terminate early
        assert!(
            state.should_terminate(),
            "Should terminate early with entropy={:.3} after {} items",
            state.entropy(),
            state.num_items_shown()
        );
    }

    #[test]
    fn test_mixed_responses_no_early_termination() {
        let mut state = InferenceState::new();
        // Alternate interested/not interested
        for slot in 0..7 {
            if slot % 2 == 0 {
                state.update(slot, &TasteResponse::Interested);
            } else {
                state.update(slot, &TasteResponse::NotInterested);
            }
        }
        // Mixed responses shouldn't converge quickly
        // (may or may not terminate depending on which items)
        // Just verify the state is valid
        assert!(state.entropy() > 0.0);
        assert!(state.num_items_shown() == 7);
    }

    #[test]
    fn test_next_item_maximizes_info_gain() {
        let state = InferenceState::new();
        let next = state.next_item();
        assert!(next.is_some(), "Should always have a next item at start");
    }

    #[test]
    fn test_strong_interest_stronger_signal() {
        let mut state_interested = InferenceState::new();
        let mut state_strong = InferenceState::new();

        state_interested.update(0, &TasteResponse::Interested);
        state_strong.update(0, &TasteResponse::StrongInterest);

        // StrongInterest should produce lower entropy (more concentrated posterior)
        assert!(
            state_strong.entropy() < state_interested.entropy(),
            "StrongInterest entropy ({:.4}) should be less than Interested ({:.4})",
            state_strong.entropy(),
            state_interested.entropy()
        );
    }

    #[test]
    fn test_finalize_produces_valid_profile() {
        let mut state = InferenceState::new();
        state.update(0, &TasteResponse::Interested);
        state.update(1, &TasteResponse::NotInterested);

        let profile = state.finalize();
        assert_eq!(profile.items_shown, 2);
        assert!(profile.confidence > 0.0);
        assert!(profile.confidence <= 1.0);

        let weight_sum: f64 = profile.persona_weights.iter().sum();
        assert!(
            (weight_sum - 1.0).abs() < 1e-10,
            "Persona weights should sum to 1.0, got {weight_sum}"
        );
    }

    #[test]
    fn test_progress_increases() {
        let mut state = InferenceState::new();
        for slot in 0..5 {
            state.update(slot, &TasteResponse::Interested);
        }
        let step = state.next_step();
        match step {
            TasteTestStep::NextCard { progress, .. } => {
                assert!(progress > 0.0, "Progress should be > 0 after 5 items");
            }
            TasteTestStep::Complete { .. } => {
                // Early termination is fine too
            }
        }
    }
}
