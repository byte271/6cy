//! # Sentinel Layer: Forward Error Correction (FEC)
//!
//! The Sentinel Layer implements high-velocity Forward Error Correction utilizing
//! Cauchy-Reed-Solomon codes over GF(2⁸). It provides planetary-scale resilience
//! by generating parity shards capable of reconstructing corrupted or missing
//! data stripes with "rigorous precision."
//!
//! ## Model
//! FEC is applied at the stripe level, transforming 'k' data shards into
//! 'k + m' total shards. This Zenith implementation guarantees recovery
//! from any 'm' shard losses (corruption or truncation).

use serde::{Deserialize, Serialize};
use std::io;

// ── Zenith Galois Field (GF(2⁸)) Engine ──────────────────────────────────────

/// GF(2⁸) primitive polynomial: x⁸ + x⁴ + x³ + x² + 1 (0x11D).
const PRIMITIVE_POLY: u32 = 0x11D;

/// High-precision GF(2⁸) multiplication using the Russian Peasant algorithm.
#[inline]
pub fn gf_mul(mut a: u8, mut b: u8) -> u8 {
    let mut res = 0u8;
    for _ in 0..8 {
        if b & 1 != 0 {
            res ^= a;
        }
        let high_bit = a & 0x80;
        a <<= 1;
        if high_bit != 0 {
            a ^= (PRIMITIVE_POLY & 0xFF) as u8;
        }
        b >>= 1;
    }
    res
}

/// GF(2⁸) inverse using Fermat's Little Theorem (a²⁵⁴).
#[inline]
pub fn gf_inv(a: u8) -> u8 {
    assert!(a != 0, "Zenith GF Engine: Division by zero");
    let mut res = a;
    for _ in 0..6 {
        res = gf_mul(res, res);
        res = gf_mul(res, a);
    }
    gf_mul(res, res)
}

// ── Sentinel Precomputed Tables ──────────────────────────────────────────────

static GF_TABLES: std::sync::OnceLock<([u8; 256], [u8; 512])> = std::sync::OnceLock::new();

fn get_sentinel_tables() -> &'static ([u8; 256], [u8; 512]) {
    GF_TABLES.get_or_init(|| {
        let mut exp = [0u8; 512];
        let mut log = [0u8; 256];
        let mut x = 1u8;
        for i in 0..255u16 {
            exp[i as usize] = x;
            exp[i as usize + 255] = x;
            log[x as usize] = i as u8;
            x = gf_mul(x, 2);
        }
        (log, exp)
    })
}

/// Accelerated GF multiplication using precomputed log/exp tables.
#[inline]
fn gf_mul_fast(a: u8, b: u8) -> u8 {
    if a == 0 || b == 0 {
        return 0;
    }
    let (log, exp) = get_sentinel_tables();
    let sum = log[a as usize] as u16 + log[b as usize] as u16;
    exp[sum as usize]
}

// ── Zenith Matrix Operations ──────────────────────────────────────────────────

/// Construct the parity rows of a systematic Cauchy encoding matrix.
fn generate_cauchy_matrix(k: usize, m: usize) -> Vec<Vec<u8>> {
    let (log, exp) = get_sentinel_tables();

    (0..m)
        .map(|i| {
            let xi = (k + i) as u8;
            (0..k)
                .map(|j| {
                    let yj = j as u8;
                    let denom = xi ^ yj;
                    let log_d = log[denom as usize] as u16;
                    exp[(255 - log_d) as usize]
                })
                .collect()
        })
        .collect()
}

// ── FEC Configuration & Headers ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FecConfig {
    pub data_shards: usize,
    pub parity_shards: usize,
}

impl Default for FecConfig {
    fn default() -> Self {
        Self {
            data_shards: 10,
            parity_shards: 4,
        }
    }
}

impl FecConfig {
    pub fn validate(&self) -> Result<(), String> {
        if self.data_shards == 0 || self.parity_shards == 0 {
            return Err("Sentinel FEC: shard counts must be non-zero".into());
        }
        if self.data_shards + self.parity_shards > 255 {
            return Err("Sentinel FEC: total shards exceed GF(2^8) field size".into());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct FecBlockHeader {
    pub stripe_id: u32,
    pub shard_index: u32,
    pub data_shards: u32,
    pub parity_shards: u32,
    pub shard_size: u32,
}

pub const FEC_HEADER_SIZE: usize = 20;

impl FecBlockHeader {
    pub fn write(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.stripe_id.to_le_bytes());
        out.extend_from_slice(&self.shard_index.to_le_bytes());
        out.extend_from_slice(&self.data_shards.to_le_bytes());
        out.extend_from_slice(&self.parity_shards.to_le_bytes());
        out.extend_from_slice(&self.shard_size.to_le_bytes());
    }

    pub fn read(buf: &[u8]) -> io::Result<Self> {
        if buf.len() < FEC_HEADER_SIZE {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "FEC Header truncated",
            ));
        }
        Ok(Self {
            stripe_id: u32::from_le_bytes(buf[0..4].try_into().unwrap()),
            shard_index: u32::from_le_bytes(buf[4..8].try_into().unwrap()),
            data_shards: u32::from_le_bytes(buf[8..12].try_into().unwrap()),
            parity_shards: u32::from_le_bytes(buf[12..16].try_into().unwrap()),
            shard_size: u32::from_le_bytes(buf[16..20].try_into().unwrap()),
        })
    }
}

// ── Zenith Resilience Pipeline ───────────────────────────────────────────────

pub struct FecStripe {
    config: FecConfig,
    shards: Vec<Vec<u8>>,
}

impl FecStripe {
    pub fn new(config: FecConfig, mut shards: Vec<Vec<u8>>) -> Self {
        assert_eq!(shards.len(), config.data_shards);
        let max_len = shards.iter().map(|s| s.len()).max().unwrap_or(0);
        for s in &mut shards {
            s.resize(max_len, 0u8);
        }
        Self { config, shards }
    }

    pub fn encode_parity(&self) -> Vec<Vec<u8>> {
        let k = self.config.data_shards;
        let m = self.config.parity_shards;
        let size = self.shards[0].len();
        let matrix = generate_cauchy_matrix(k, m);

        (0..m)
            .map(|i| {
                let row = &matrix[i];
                let mut p = vec![0u8; size];
                for (j, shard) in self.shards.iter().enumerate() {
                    let coeff = row[j];
                    for (b, &val) in shard.iter().enumerate() {
                        p[b] ^= gf_mul_fast(coeff, val);
                    }
                }
                p
            })
            .collect()
    }

    pub fn reconstruct(
        cfg: &FecConfig,
        available: Vec<Option<Vec<u8>>>,
    ) -> Result<Vec<Vec<u8>>, String> {
        let k = cfg.data_shards;
        let m = cfg.parity_shards;

        if available.len() != k + m {
            return Err("Shard vector mismatch".into());
        }
        if available[..k].iter().all(|s| s.is_some()) {
            return Ok(available.into_iter().take(k).map(|s| s.unwrap()).collect());
        }

        let present: Vec<(usize, &Vec<u8>)> = available
            .iter()
            .enumerate()
            .filter_map(|(i, s)| s.as_ref().map(|d| (i, d)))
            .collect();

        if present.len() < k {
            return Err("Insufficient shards for Zenith recovery".into());
        }

        let chosen = &present[..k];
        let size = chosen[0].1.len();
        let matrix = generate_cauchy_matrix(k, m);

        let full_matrix: Vec<Vec<u8>> = (0..k + m)
            .map(|i| {
                if i < k {
                    let mut r = vec![0u8; k];
                    r[i] = 1;
                    r
                } else {
                    matrix[i - k].clone()
                }
            })
            .collect();

        let sub_matrix: Vec<Vec<u8>> = chosen
            .iter()
            .map(|&(idx, _)| full_matrix[idx].clone())
            .collect();
        let inv = invert_matrix(sub_matrix, k).ok_or("Parity singular")?;

        let mut res = vec![vec![0u8; size]; k];
        for (out_r, inv_r) in res.iter_mut().zip(inv.iter()) {
            for (col, &(_, shard)) in chosen.iter().enumerate() {
                let coeff = inv_r[col];
                for (b, &val) in shard.iter().enumerate() {
                    out_r[b] ^= gf_mul_fast(coeff, val);
                }
            }
        }
        Ok(res)
    }
}

fn invert_matrix(m: Vec<Vec<u8>>, n: usize) -> Option<Vec<Vec<u8>>> {
    let mut aug: Vec<Vec<u8>> = (0..n)
        .map(|i| {
            let mut r = m[i].clone();
            r.extend((0..n).map(|j| if i == j { 1u8 } else { 0u8 }));
            r
        })
        .collect();

    for col in 0..n {
        let pivot_row = (col..n).find(|&r| aug[r][col] != 0)?;
        aug.swap(col, pivot_row);
        let inv_pivot = gf_inv(aug[col][col]);
        for val in aug[col].iter_mut() {
            *val = gf_mul_fast(*val, inv_pivot);
        }
        for r in 0..n {
            if r == col {
                continue;
            }
            let f = aug[r][col];
            if f != 0 {
                let pivot_copy = aug[col].clone();
                for (val, &pv) in aug[r].iter_mut().zip(pivot_copy.iter()) {
                    *val ^= gf_mul_fast(f, pv);
                }
            }
        }
    }
    Some(aug.into_iter().map(|r| r[n..].to_vec()).collect())
}

pub fn encode_parity_shards(cfg: &FecConfig, data: Vec<Vec<u8>>) -> Result<Vec<Vec<u8>>, String> {
    if data.len() != cfg.data_shards {
        return Err("Shard count mismatch".into());
    }
    Ok(FecStripe::new(cfg.clone(), data).encode_parity())
}

pub fn reconstruct_data_shards(
    cfg: &FecConfig,
    available: Vec<Option<Vec<u8>>>,
) -> Result<Vec<Vec<u8>>, String> {
    FecStripe::reconstruct(cfg, available)
}
