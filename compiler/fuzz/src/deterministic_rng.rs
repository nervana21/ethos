//! Deterministic Random Number Generator for Fuzzing
//!
//! This module provides a deterministic RNG system that uses a seed from the
//! FUZZ_SEED environment variable to ensure reproducible fuzzing runs.

use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha20Rng;
use std::sync::Mutex;

/// Global deterministic RNG instance
static DETERMINISTIC_RNG: Mutex<Option<ChaCha20Rng>> = Mutex::new(None);

/// Initialize the deterministic RNG with a seed from environment
pub fn init_deterministic_rng() -> Result<u64, std::num::ParseIntError> {
    let seed = std::env::var("FUZZ_SEED")
        .unwrap_or_else(|_| "42".to_string())
        .parse::<u64>()?;

    let mut rng = ChaCha20Rng::seed_from_u64(seed);
    let mut global_rng = DETERMINISTIC_RNG.lock().unwrap();
    *global_rng = Some(rng);

    Ok(seed)
}

/// Initialize the deterministic RNG with a seed from input data
pub fn init_with_seed(seed_data: &[u8]) {
    // Create a seed from the input data by hashing it
    use sha2::{Sha256, Digest};
    let mut hasher = Sha256::new();
    hasher.update(seed_data);
    let hash = hasher.finalize();

    // Use first 8 bytes of hash as u64 seed
    let seed = u64::from_le_bytes([
        hash[0], hash[1], hash[2], hash[3],
        hash[4], hash[5], hash[6], hash[7]
    ]);

    let mut rng = ChaCha20Rng::seed_from_u64(seed);
    let mut global_rng = DETERMINISTIC_RNG.lock().unwrap();
    *global_rng = Some(rng);
}

// /// Get a random value from the deterministic RNG
// pub fn random<T>() -> T
// where
//     T: rand::distributions::Distribution<T> + Copy,
//     ChaCha20Rng: rand::RngCore,
// {
//     let mut global_rng = DETERMINISTIC_RNG.lock().unwrap();
//     if let Some(ref mut rng) = *global_rng {
//         rng.gen()
//     } else {
//         // Fallback to system RNG if not initialized
//         rand::rng().gen()
//     }
// }

/// Get a random number in a range
pub fn random_range(min: u64, max: u64) -> u64 {
    let mut global_rng = DETERMINISTIC_RNG.lock().unwrap();
    if let Some(ref mut rng) = *global_rng {
        rng.gen_range(min..=max)
    } else {
        rand::random::<u64>() % (max - min + 1) + min
    }
}

/// Get a random boolean
pub fn random_bool() -> bool {
    let mut global_rng = DETERMINISTIC_RNG.lock().unwrap();
    if let Some(ref mut rng) = *global_rng {
        rng.gen()
    } else {
        rand::random()
    }
}

/// Get a random string of specified length
pub fn random_string(length: usize) -> String {
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    let mut global_rng = DETERMINISTIC_RNG.lock().unwrap();
    if let Some(ref mut rng) = *global_rng {
        (0..length)
            .map(|_| {
                let idx = rng.gen_range(0..CHARSET.len());
                CHARSET[idx] as char
            })
            .collect()
    } else {
        // Fallback: generate random string using system RNG
        (0..length)
            .map(|_| {
                let idx = (rand::random::<u32>() as usize) % CHARSET.len();
                CHARSET[idx] as char
            })
            .collect()
    }
}

/// Get a random array of bytes
pub fn random_bytes(length: usize) -> Vec<u8> {
    let mut global_rng = DETERMINISTIC_RNG.lock().unwrap();
    if let Some(ref mut rng) = *global_rng {
        (0..length).map(|_| rng.gen()).collect()
    } else {
        (0..length).map(|_| rand::random::<u8>()).collect()
    }
}

/// Get a random choice from a slice
pub fn random_choice<T>(choices: &[T]) -> Option<&T> {
    if choices.is_empty() {
        return None;
    }

    let mut global_rng = DETERMINISTIC_RNG.lock().unwrap();
    if let Some(ref mut rng) = *global_rng {
        let idx = rng.gen_range(0..choices.len());
        Some(&choices[idx])
    } else {
        let idx = (rand::random::<u32>() as usize) % choices.len();
        Some(&choices[idx])
    }
}

/// Get the current seed (for debugging)
pub fn get_current_seed() -> Option<u64> {
    std::env::var("FUZZ_SEED")
        .ok()
        .and_then(|s| s.parse().ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deterministic_rng_initialization() {
        std::env::set_var("FUZZ_SEED", "12345");
        let seed = init_deterministic_rng().unwrap();
        assert_eq!(seed, 12345);

        // Clean up
        std::env::remove_var("FUZZ_SEED");
    }

    #[test]
    fn test_deterministic_rng_default_seed() {
        std::env::remove_var("FUZZ_SEED");
        let seed = init_deterministic_rng().unwrap();
        assert_eq!(seed, 42);
    }

    #[test]
    fn test_random_choice() {
        init_deterministic_rng().unwrap();
        let choices = vec!["a", "b", "c"];
        let choice = random_choice(&choices).unwrap();
        assert!(choices.contains(choice));
    }
}
