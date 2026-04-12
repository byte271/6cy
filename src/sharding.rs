//! # Nebula Layer: Cryptographic Sharding
//!
//! The Nebula Layer implements Distributed Sovereignty through Shamir's Secret
//! Sharing (SSS) over GF(2⁸). It allows for the cryptographic fragmentation
//! of master keys and data volumes across N physical entities, requiring a
//! threshold of K shards for reconstruction.
//!
//! ## Theory of Distributed Sovereignty
//! Utilizing Lagrange Polynomial Interpolation, a secret is encoded as the
//! constant term $P(0)$ of a random polynomial of degree $K-1$. The shards
//! are evaluations of this polynomial at $N$ distinct non-zero points.

use rand::{RngCore, SeedableRng};
use rand_chacha::ChaCha8Rng;

/// A single fragment of a shared secret.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct NebulaShard {
    /// The X-coordinate (index) of the shard.
    pub x: u8,
    /// The Y-coordinate (the value of the polynomial at X).
    pub y: Vec<u8>,
}

/// The Zenith-grade splitter for master keys and data streams.
pub struct NebulaSplitter {
    k: u8,
    n: u8,
}

impl NebulaSplitter {
    /// Create a new splitter with threshold K and total shards N.
    pub fn new(k: u8, n: u8) -> Result<Self, String> {
        if k == 0 || n == 0 || k > n {
            return Err("Invalid threshold: must satisfy 0 < K <= N".into());
        }
        Ok(Self { k, n })
    }

    /// Fragments a secret into N NebulaShards.
    pub fn split(&self, secret: &[u8]) -> Vec<NebulaShard> {
        let mut shards = Vec::with_capacity(self.n as usize);
        for x in 1..=self.n {
            shards.push(NebulaShard {
                x,
                y: vec![0u8; secret.len()],
            });
        }

        let mut rng = ChaCha8Rng::from_entropy();

        for (byte_idx, &secret_byte) in secret.iter().enumerate() {
            // Generate a random polynomial of degree K-1: P(x) = a0 + a1*x + a2*x^2 + ...
            // a0 is the secret byte.
            let mut coeffs = vec![0u8; self.k as usize];
            coeffs[0] = secret_byte;
            rng.fill_bytes(&mut coeffs[1..]);

            // Evaluate P(x) for each x in 1..=N
            for x_idx in 1..=self.n {
                let mut y = 0u8;
                let mut x_pow = 1u8; // x^0, x^1, x^2...

                for &coeff in &coeffs {
                    // y += coeff * x^i
                    let term = fec_mul(coeff, x_pow);
                    y ^= term; // Addition in GF(2^8) is XOR
                    x_pow = fec_mul(x_pow, x_idx);
                }
                shards[(x_idx - 1) as usize].y[byte_idx] = y;
            }
        }

        shards
    }
}

/// The Zenith-grade reconstructor for restoring secrets from fragments.
pub struct NebulaReconstructor {
    k: u8,
}

impl NebulaReconstructor {
    pub fn new(k: u8) -> Self {
        Self { k }
    }

    /// Reconstructs the original secret from at least K NebulaShards.
    pub fn reconstruct(&self, shards: &[NebulaShard]) -> Result<Vec<u8>, String> {
        if shards.len() < self.k as usize {
            return Err(format!(
                "Insufficient shards: have {}, need {}",
                shards.len(),
                self.k
            ));
        }

        let secret_len = shards[0].y.len();
        let mut secret = vec![0u8; secret_len];

        for byte_idx in 0..secret_len {
            let mut val = 0u8;

            // Use Lagrange interpolation at x=0 to find the constant term (the secret).
            // L(x) = SUM( y_i * PROD( (x - x_j) / (x_i - x_j) ) ) for i != j
            // For x=0: L(0) = SUM( y_i * PROD( x_j / (x_j - x_i) ) )
            for i in 0..self.k as usize {
                let xi = shards[i].x;
                let yi = shards[i].y[byte_idx];

                let mut li = 1u8;
                for j in 0..self.k as usize {
                    if i == j {
                        continue;
                    }
                    let xj = shards[j].x;

                    // num = xj, den = xj - xi
                    let num = xj;
                    let den = xj ^ xi;
                    li = fec_mul(li, fec_mul(num, fec_inv(den)));
                }
                val ^= fec_mul(yi, li);
            }
            secret[byte_idx] = val;
        }

        Ok(secret)
    }
}

// ── Internal Math Bridge ─────────────────────────────────────────────────────

/// Multiplication in GF(2^8). Direct wrap of the FEC module for Zenith precision.
fn fec_mul(a: u8, b: u8) -> u8 {
    // In a real optimized build, we'd use the table lookup here.
    // Re-implementing Russian Peasant for standalone reliability in this module.
    let mut a = a;
    let mut b = b;
    let mut res = 0u8;
    for _ in 0..8 {
        if b & 1 != 0 {
            res ^= a;
        }
        let high_bit = a & 0x80;
        a <<= 1;
        if high_bit != 0 {
            a ^= 0x1D;
        } // x^8 + x^4 + x^3 + x^2 + 1
        b >>= 1;
    }
    res
}

/// Inverse in GF(2^8).
fn fec_inv(a: u8) -> u8 {
    if a == 0 {
        panic!("GF(2^8) Division by zero");
    }
    // a^254 is the inverse
    let mut res = a;
    for _ in 0..6 {
        res = fec_mul(res, res);
        res = fec_mul(res, a);
    }
    fec_mul(res, res)
}
