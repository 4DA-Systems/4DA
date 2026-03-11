//! Team sync encryption -- XChaCha20Poly1305 + X25519 key exchange.
//!
//! The relay server stores only encrypted blobs and cannot read team metadata.
//! All encryption/decryption happens on the client.
//!
//! Key hierarchy:
//! - Each member has an X25519 keypair (generated on team join)
//! - A team-wide symmetric key is derived and distributed by the admin
//! - All sync entries are encrypted with the team symmetric key
//! - The team key is encrypted per-member using X25519 for distribution

use anyhow::{bail, Context, Result};
use chacha20poly1305::{
    aead::{Aead, AeadCore, KeyInit, OsRng},
    XChaCha20Poly1305, XNonce,
};
use hkdf::Hkdf;
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use tracing::info;
use x25519_dalek::{PublicKey, StaticSecret};

/// Size of XChaCha20Poly1305 nonce (24 bytes).
const NONCE_SIZE: usize = 24;

/// Team crypto state for a single team membership.
///
/// Manual `Debug` impl omits the private key to prevent accidental leakage
/// in logs or error messages.
pub struct TeamCrypto {
    /// Our X25519 public key (shareable).
    pub our_public_key: PublicKey,
    /// Our X25519 private key (never leaves this machine).
    our_private_key: StaticSecret,
    /// Team-wide symmetric encryption key (32 bytes).
    /// None until team key is received from admin.
    team_key: Option<[u8; 32]>,
}

impl std::fmt::Debug for TeamCrypto {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TeamCrypto")
            .field(
                "our_public_key",
                &hex::encode(self.our_public_key.as_bytes()),
            )
            .field("our_private_key", &"[REDACTED]")
            .field("has_team_key", &self.team_key.is_some())
            .finish()
    }
}

/// Serializable keypair for storage in the `team_crypto` table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorableKeypair {
    pub public_key: Vec<u8>,
    pub private_key: Vec<u8>,
}

impl TeamCrypto {
    /// Generate a new X25519 keypair for this team member.
    pub fn generate() -> Self {
        let secret = StaticSecret::random_from_rng(OsRng);
        let public = PublicKey::from(&secret);

        info!(target: "4da::team_crypto", "Generated new X25519 keypair");

        Self {
            our_public_key: public,
            our_private_key: secret,
            team_key: None,
        }
    }

    /// Restore from stored keypair bytes.
    pub fn from_stored(public_bytes: &[u8; 32], private_bytes: &[u8; 32]) -> Self {
        let secret = StaticSecret::from(*private_bytes);
        let public = PublicKey::from(*public_bytes);

        Self {
            our_public_key: public,
            our_private_key: secret,
            team_key: None,
        }
    }

    /// Get the raw bytes of our public key (for sharing with relay/peers).
    pub fn public_key_bytes(&self) -> [u8; 32] {
        *self.our_public_key.as_bytes()
    }

    /// Get the raw bytes of our private key (for secure local storage).
    pub fn private_key_bytes(&self) -> [u8; 32] {
        self.our_private_key.to_bytes()
    }

    /// Derive a shared secret with a peer using X25519 Diffie-Hellman,
    /// then expand it via HKDF into a usable encryption key.
    pub fn derive_shared_key(&self, peer_public: &PublicKey) -> [u8; 32] {
        let shared_secret = self.our_private_key.diffie_hellman(peer_public);

        // Expand raw DH output via HKDF-SHA256
        let hk = Hkdf::<Sha256>::new(None, shared_secret.as_bytes());
        let mut okm = [0u8; 32];
        hk.expand(b"4da-team-sync-v1", &mut okm)
            .expect("HKDF expand should not fail with 32-byte output");
        okm
    }

    /// Set the team-wide symmetric key (received from admin during join).
    pub fn set_team_key(&mut self, key: [u8; 32]) {
        self.team_key = Some(key);
    }

    /// Get the team key (returns None if not yet received).
    pub fn team_key(&self) -> Option<&[u8; 32]> {
        self.team_key.as_ref()
    }

    /// Check if this crypto instance has a usable team key.
    pub fn has_team_key(&self) -> bool {
        self.team_key.is_some()
    }

    /// Generate a new random team-wide symmetric key.
    /// Called by the admin when creating a team.
    pub fn generate_team_key() -> [u8; 32] {
        let mut key = [0u8; 32];
        use rand::RngCore;
        OsRng.fill_bytes(&mut key);
        key
    }

    /// Encrypt the team key for a specific member using their public key.
    /// Used by admin to distribute the team key to new members.
    pub fn encrypt_team_key_for_member(
        &self,
        team_key: &[u8; 32],
        member_public: &PublicKey,
    ) -> Result<Vec<u8>> {
        let shared = self.derive_shared_key(member_public);
        encrypt_bytes(&shared, team_key)
    }

    /// Decrypt the team key received from admin.
    /// The team key was encrypted using our public key.
    pub fn decrypt_team_key_from_admin(
        &mut self,
        encrypted_team_key: &[u8],
        admin_public: &PublicKey,
    ) -> Result<()> {
        let shared = self.derive_shared_key(admin_public);
        let decrypted = decrypt_bytes(&shared, encrypted_team_key)?;

        if decrypted.len() != 32 {
            bail!(
                "Decrypted team key has wrong length: {} (expected 32)",
                decrypted.len()
            );
        }

        let mut key = [0u8; 32];
        key.copy_from_slice(&decrypted);
        self.team_key = Some(key);

        info!(target: "4da::team_crypto", "Team key decrypted and stored");
        Ok(())
    }

    /// Export the keypair as a serializable struct for database storage.
    pub fn to_storable(&self) -> StorableKeypair {
        StorableKeypair {
            public_key: self.public_key_bytes().to_vec(),
            private_key: self.private_key_bytes().to_vec(),
        }
    }
}

/// Encrypt a metadata payload using the team symmetric key.
///
/// Format: `[24-byte nonce][ciphertext + 16-byte Poly1305 tag]`
pub fn encrypt_metadata(team_key: &[u8; 32], plaintext: &[u8]) -> Result<Vec<u8>> {
    encrypt_bytes(team_key, plaintext)
}

/// Decrypt a metadata payload using the team symmetric key.
pub fn decrypt_metadata(team_key: &[u8; 32], blob: &[u8]) -> Result<Vec<u8>> {
    decrypt_bytes(team_key, blob)
}

/// Encrypt a `TeamMetadataEntry` to an opaque blob for the relay.
pub fn encrypt_entry(
    team_key: &[u8; 32],
    entry: &crate::team_sync_types::TeamMetadataEntry,
) -> Result<Vec<u8>> {
    let plaintext = serde_json::to_vec(entry).context("Failed to serialize TeamMetadataEntry")?;
    encrypt_metadata(team_key, &plaintext)
}

/// Decrypt an opaque blob from the relay into a `TeamMetadataEntry`.
pub fn decrypt_entry(
    team_key: &[u8; 32],
    blob: &[u8],
) -> Result<crate::team_sync_types::TeamMetadataEntry> {
    let plaintext = decrypt_metadata(team_key, blob)?;
    let entry =
        serde_json::from_slice(&plaintext).context("Failed to deserialize TeamMetadataEntry")?;
    Ok(entry)
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

fn encrypt_bytes(key: &[u8; 32], plaintext: &[u8]) -> Result<Vec<u8>> {
    let cipher = XChaCha20Poly1305::new(key.into());
    let nonce = XChaCha20Poly1305::generate_nonce(&mut OsRng);

    let ciphertext = cipher
        .encrypt(&nonce, plaintext)
        .map_err(|e| anyhow::anyhow!("Encryption failed: {e}"))?;

    // Prepend nonce: [24 bytes nonce][N bytes ciphertext+tag]
    let mut result = Vec::with_capacity(NONCE_SIZE + ciphertext.len());
    result.extend_from_slice(&nonce);
    result.extend_from_slice(&ciphertext);
    Ok(result)
}

fn decrypt_bytes(key: &[u8; 32], blob: &[u8]) -> Result<Vec<u8>> {
    if blob.len() < NONCE_SIZE {
        bail!(
            "Encrypted blob too short ({} bytes, need at least {})",
            blob.len(),
            NONCE_SIZE
        );
    }

    let (nonce_bytes, ciphertext) = blob.split_at(NONCE_SIZE);
    let nonce = XNonce::from_slice(nonce_bytes);

    let cipher = XChaCha20Poly1305::new(key.into());
    let plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|_| anyhow::anyhow!("Decryption failed (wrong key or tampered data)"))?;

    Ok(plaintext)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keypair_generation_produces_nonzero_keys() {
        let crypto = TeamCrypto::generate();
        let pub_bytes = crypto.public_key_bytes();
        assert_ne!(pub_bytes, [0u8; 32], "Public key should not be all zeros");
    }

    #[test]
    fn keypair_roundtrip_through_storage() {
        let original = TeamCrypto::generate();
        let pub_bytes = original.public_key_bytes();
        let priv_bytes = original.private_key_bytes();

        let restored = TeamCrypto::from_stored(&pub_bytes, &priv_bytes);
        assert_eq!(
            restored.public_key_bytes(),
            pub_bytes,
            "Restored public key should match original"
        );
    }

    #[test]
    fn x25519_shared_key_agreement() {
        let alice = TeamCrypto::generate();
        let bob = TeamCrypto::generate();

        let alice_shared = alice.derive_shared_key(&bob.our_public_key);
        let bob_shared = bob.derive_shared_key(&alice.our_public_key);

        assert_eq!(
            alice_shared, bob_shared,
            "Shared keys must match (DH symmetry)"
        );
    }

    #[test]
    fn encrypt_decrypt_roundtrip() {
        let key = TeamCrypto::generate_team_key();
        let plaintext = b"Hello, team!";

        let encrypted = encrypt_metadata(&key, plaintext).unwrap();
        assert_ne!(
            &encrypted[..],
            &plaintext[..],
            "Ciphertext must differ from plaintext"
        );
        assert!(
            encrypted.len() > plaintext.len(),
            "Encrypted output includes nonce + tag overhead"
        );

        let decrypted = decrypt_metadata(&key, &encrypted).unwrap();
        assert_eq!(decrypted, plaintext, "Decrypted output must match original");
    }

    #[test]
    fn decrypt_with_wrong_key_fails() {
        let key1 = TeamCrypto::generate_team_key();
        let key2 = TeamCrypto::generate_team_key();

        let encrypted = encrypt_metadata(&key1, b"secret").unwrap();
        let result = decrypt_metadata(&key2, &encrypted);

        assert!(result.is_err(), "Decryption with wrong key must fail");
    }

    #[test]
    fn decrypt_tampered_data_fails() {
        let key = TeamCrypto::generate_team_key();
        let mut encrypted = encrypt_metadata(&key, b"secret").unwrap();

        // Flip the last byte of the ciphertext (inside the Poly1305 tag region)
        if let Some(byte) = encrypted.last_mut() {
            *byte ^= 0xFF;
        }

        let result = decrypt_metadata(&key, &encrypted);
        assert!(
            result.is_err(),
            "Authenticated decryption must reject tampered data"
        );
    }

    #[test]
    fn decrypt_too_short_blob_fails() {
        let key = TeamCrypto::generate_team_key();
        let result = decrypt_metadata(&key, &[0u8; 10]);
        assert!(
            result.is_err(),
            "Blob shorter than nonce size must be rejected"
        );
    }

    #[test]
    fn team_key_distribution_admin_to_member() {
        let mut admin = TeamCrypto::generate();
        let mut member = TeamCrypto::generate();

        // Admin generates and stores the team key
        let team_key = TeamCrypto::generate_team_key();
        admin.set_team_key(team_key);

        // Admin encrypts the team key for the member
        let encrypted_for_member = admin
            .encrypt_team_key_for_member(&team_key, &member.our_public_key)
            .unwrap();

        // Member decrypts team key from admin
        member
            .decrypt_team_key_from_admin(&encrypted_for_member, &admin.our_public_key)
            .unwrap();

        assert_eq!(
            admin.team_key().unwrap(),
            member.team_key().unwrap(),
            "Admin and member must share the same team key"
        );
    }

    #[test]
    fn entry_encrypt_decrypt_roundtrip() {
        let key = TeamCrypto::generate_team_key();

        let entry = crate::team_sync_types::TeamMetadataEntry {
            entry_id: "test-123".to_string(),
            client_id: "client-456".to_string(),
            hlc_timestamp: 1_234_567_890,
            operation: crate::team_sync_types::TeamOp::ShareDnaSummary {
                primary_stack: vec!["rust".to_string(), "typescript".to_string()],
                interests: vec!["systems".to_string()],
                blind_spots: vec!["kubernetes".to_string()],
                identity_summary: "Rust/TS systems developer".to_string(),
            },
        };

        let encrypted = encrypt_entry(&key, &entry).unwrap();
        let decrypted = decrypt_entry(&key, &encrypted).unwrap();

        assert_eq!(decrypted.entry_id, entry.entry_id);
        assert_eq!(decrypted.client_id, entry.client_id);
        assert_eq!(decrypted.hlc_timestamp, entry.hlc_timestamp);
    }

    #[test]
    fn different_nonces_for_same_plaintext() {
        let key = TeamCrypto::generate_team_key();
        let plaintext = b"same data";

        let enc1 = encrypt_metadata(&key, plaintext).unwrap();
        let enc2 = encrypt_metadata(&key, plaintext).unwrap();

        // Each encryption must use a unique random nonce (first 24 bytes)
        assert_ne!(
            &enc1[..NONCE_SIZE],
            &enc2[..NONCE_SIZE],
            "Each encryption must use a unique random nonce"
        );

        // Both must decrypt to the same plaintext
        assert_eq!(
            decrypt_metadata(&key, &enc1).unwrap(),
            decrypt_metadata(&key, &enc2).unwrap(),
            "Both encryptions must yield the same plaintext"
        );
    }

    #[test]
    fn storable_keypair_roundtrip() {
        let original = TeamCrypto::generate();
        let storable = original.to_storable();

        assert_eq!(storable.public_key.len(), 32);
        assert_eq!(storable.private_key.len(), 32);

        let pub_arr: [u8; 32] = storable.public_key.as_slice().try_into().unwrap();
        let priv_arr: [u8; 32] = storable.private_key.as_slice().try_into().unwrap();
        let restored = TeamCrypto::from_stored(&pub_arr, &priv_arr);

        assert_eq!(restored.public_key_bytes(), original.public_key_bytes());
    }
}
