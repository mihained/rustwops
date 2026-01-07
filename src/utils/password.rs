use rand::Rng;

const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789!@#$%^&*";
const CHARSET_ALPHANUM: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";

/// Generate a random password
pub fn generate(length: usize) -> String {
    let mut rng = rand::thread_rng();

    (0..length)
        .map(|_| {
            let idx = rng.gen_range(0..CHARSET_ALPHANUM.len());
            CHARSET_ALPHANUM[idx] as char
        })
        .collect()
}

/// Generate a random password with special characters
pub fn generate_strong(length: usize) -> String {
    let mut rng = rand::thread_rng();

    (0..length)
        .map(|_| {
            let idx = rng.gen_range(0..CHARSET.len());
            CHARSET[idx] as char
        })
        .collect()
}

/// Generate a URL-safe random string
pub fn generate_urlsafe(length: usize) -> String {
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};

    let mut bytes = vec![0u8; length];
    rand::thread_rng().fill(&mut bytes[..]);

    URL_SAFE_NO_PAD.encode(&bytes)[..length].to_string()
}
