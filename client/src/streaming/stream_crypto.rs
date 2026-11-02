//! Authentification et chiffrement du flux CoopLink.
//!
//! Le tunnel UDP était déjà chiffré ; ce module couvre le chemin de diffusion
//! (vidéo + audio) qui circulait en clair et acceptait n'importe quel pair.
//!
//! - La clé est dérivée du code de salon et du mot de passe (SHA-256 itéré).
//! - L'invité prouve qu'il connaît la clé avec un jeton HMAC avant toute trame.
//! - Chaque trame est scellée en ChaCha20-Poly1305 (nonce aléatoire par trame).

use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{ChaCha20Poly1305, Nonce};
use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};

const KDF_ROUNDS: usize = 50_000;
pub const AUTH_LABEL: &[u8] = b"cooplink-stream-auth-v1";
pub const NONCE_LEN: usize = 12;

/// Dérive la clé de session à partir du code de salon et du mot de passe.
///
/// Le code de salon est court : les tours de hachage rendent une attaque par
/// dictionnaire nettement plus coûteuse qu'une simple comparaison.
pub fn derive_key(room_code: &str, password: &str) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(b"cooplink-stream-key-v1");
    hasher.update(room_code.trim().to_uppercase().as_bytes());
    hasher.update([0u8]);
    hasher.update(password.as_bytes());
    let mut digest = hasher.finalize();

    for _ in 0..KDF_ROUNDS {
        let mut round = Sha256::new();
        round.update(digest);
        round.update(b"cooplink-kdf");
        digest = round.finalize();
    }

    let mut key = [0u8; 32];
    key.copy_from_slice(&digest);
    key
}

/// Jeton que l'invité envoie pour prouver qu'il connaît la clé.
pub fn auth_token(key: &[u8; 32]) -> [u8; 32] {
    let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(key).expect("HMAC accepte toute clé");
    mac.update(AUTH_LABEL);
    let out = mac.finalize().into_bytes();
    let mut token = [0u8; 32];
    token.copy_from_slice(&out);
    token
}

/// Chiffrement symétrique des trames.
pub struct StreamCipher {
    aead: ChaCha20Poly1305,
}

impl StreamCipher {
    pub fn new(key: &[u8; 32]) -> Self {
        Self {
            aead: ChaCha20Poly1305::new(key.into()),
        }
    }

    /// `nonce (12) || ciphertext + tag`
    pub fn seal(&self, plaintext: &[u8]) -> Option<Vec<u8>> {
        let nonce_bytes: [u8; NONCE_LEN] = rand::random();
        let nonce = Nonce::from_slice(&nonce_bytes);
        let sealed = self.aead.encrypt(nonce, plaintext).ok()?;
        let mut out = Vec::with_capacity(NONCE_LEN + sealed.len());
        out.extend_from_slice(&nonce_bytes);
        out.extend_from_slice(&sealed);
        Some(out)
    }

    pub fn open(&self, payload: &[u8]) -> Option<Vec<u8>> {
        if payload.len() <= NONCE_LEN {
            return None;
        }
        let (nonce_bytes, ciphertext) = payload.split_at(NONCE_LEN);
        let nonce = Nonce::from_slice(nonce_bytes);
        self.aead.decrypt(nonce, ciphertext).ok()
    }
}

/// Comparaison à temps constant (évite de fuiter le jeton octet par octet).
pub fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

pub fn to_hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push_str(&format!("{:02x}", b));
    }
    out
}

pub fn from_hex(text: &str) -> Option<Vec<u8>> {
    let clean: Vec<char> = text.trim().chars().filter(|c| !c.is_whitespace()).collect();
    if clean.len() % 2 != 0 {
        return None;
    }
    let mut out = Vec::with_capacity(clean.len() / 2);
    let mut i = 0;
    while i < clean.len() {
        let hi = clean[i].to_digit(16)?;
        let lo = clean[i + 1].to_digit(16)?;
        out.push(((hi << 4) | lo) as u8);
        i += 2;
    }
    Some(out)
}

/// Code de salon à 8 caractères (32^8 combinaisons), au lieu de 4 chiffres.
pub fn random_room_code() -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789"; // sans I/O/0/1
    let mut code = String::from("COOP-");
    for _ in 0..8 {
        let idx = (rand::random::<u32>() as usize) % ALPHABET.len();
        code.push(ALPHABET[idx] as char);
    }
    code
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_seal_open() {
        let key = derive_key("COOP-ABCD1234", "secret");
        let cipher = StreamCipher::new(&key);
        let sealed = cipher.seal(b"frame video").expect("seal");
        assert_ne!(&sealed[..], b"frame video");
        assert_eq!(cipher.open(&sealed).expect("open"), b"frame video");
    }

    #[test]
    fn tampering_is_rejected() {
        let key = derive_key("COOP-ABCD1234", "");
        let cipher = StreamCipher::new(&key);
        let mut sealed = cipher.seal(b"payload").expect("seal");
        let last = sealed.len() - 1;
        sealed[last] ^= 0x01;
        assert!(cipher.open(&sealed).is_none());
    }

    #[test]
    fn wrong_key_cannot_open() {
        let a = StreamCipher::new(&derive_key("COOP-AAAA", ""));
        let b = StreamCipher::new(&derive_key("COOP-BBBB", ""));
        let sealed = a.seal(b"confidentiel").expect("seal");
        assert!(b.open(&sealed).is_none());
    }

    #[test]
    fn key_depends_on_room_and_password() {
        assert_eq!(derive_key("coop-abcd", "x"), derive_key("COOP-ABCD", "x"));
        assert_ne!(derive_key("COOP-ABCD", "x"), derive_key("COOP-ABCD", "y"));
        assert_ne!(derive_key("COOP-ABCD", "x"), derive_key("COOP-ABCD", ""));
    }

    #[test]
    fn auth_token_is_stable_and_key_dependent() {
        let key = derive_key("COOP-ABCD", "");
        assert_eq!(auth_token(&key), auth_token(&key));
        assert_ne!(auth_token(&key), auth_token(&derive_key("COOP-EFGH", "")));
    }

    #[test]
    fn hex_roundtrip_and_room_code_shape() {
        let raw = [0u8, 1, 15, 16, 255];
        assert_eq!(from_hex(&to_hex(&raw)).unwrap(), raw.to_vec());
        assert!(from_hex("zz").is_none());

        for _ in 0..20 {
            let code = random_room_code();
            assert_eq!(code.len(), 13); // "COOP-" + 8
            assert!(code.starts_with("COOP-"));
            assert!(code[5..].chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit()));
        }
    }

    #[test]
    fn constant_time_eq_behaves() {
        assert!(constant_time_eq(b"abc", b"abc"));
        assert!(!constant_time_eq(b"abc", b"abd"));
        assert!(!constant_time_eq(b"abc", b"ab"));
    }
}
