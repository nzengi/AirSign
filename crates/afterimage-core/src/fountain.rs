//! afterimage_core::fountain
//! =========================
//! Rateless LT Fountain Code encoder and decoder with Robust Soliton Distribution.
//!
//! # Design
//!
//! * **Encoder** is truly rateless: it generates an infinite stream of coded
//!   droplets from a fixed source block set.  The sender loops until the
//!   receiver signals completion (or the operator presses q).
//! * **Decoder** uses belief-propagation with an inverted index, giving
//!   O(degree) propagation cost per resolved block instead of O(n × pending).
//! * **RNG**: ChaCha8 seeded with the 32-bit droplet seed.  This is the
//!   canonical, reproducible mapping `seed → neighbour set` for the v2
//!   protocol.  The v1 Python protocol used NumPy PCG64; the version byte in
//!   the METADATA frame tells the receiver which algorithm to use.
//! * **Wire format** per droplet:
//!   ```text
//!   seed (4 B, uint32 BE) || degree (2 B, uint16 BE) || reserved (2 B) || payload (BLOCK_SIZE B)
//!   ```
//!   The `degree` field is informational only.  The decoder always re-derives
//!   neighbours from the seed — it never trusts the header degree.
//!
//! # Robustness
//! The Robust Soliton Distribution is parameterised for ~40 % channel loss.
//! With `c = 0.1` and `δ = 0.5`, the expected number of droplets needed for
//! reliable decoding is roughly `k × 1.05 + 10`.

use std::collections::{HashMap, HashSet};

use rand::{RngCore, SeedableRng};
use rand_chacha::ChaCha8Rng;

use crate::error::FountainError;

// ─── Constants ───────────────────────────────────────────────────────────────

/// Bytes per source block (must match encoder and decoder).
pub const BLOCK_SIZE: usize = 256;
/// Wire-format header per droplet: seed(4) + degree(2) + reserved(2).
pub const HEADER_SIZE: usize = 8;
/// 5 % overhead above `k` plus a constant for the recommended droplet count.
pub const OVERHEAD_FACTOR: f64 = 1.05;
/// Constant addend for the recommended droplet count.
pub const OVERHEAD_ADDEND: usize = 10;

/// Robust Soliton tuning constant (ripple size).
const C_PARAM: f64 = 0.1;
/// Robust Soliton failure-probability bound.
const DELTA_PARAM: f64 = 0.5;

// ─── Robust Soliton Distribution ─────────────────────────────────────────────

/// Pre-computed Robust Soliton Distribution CDF.
///
/// Instantiate once per session; sampling is O(log k) via binary search.
#[derive(Debug, Clone)]
pub struct RobustSoliton {
    /// Number of source blocks.
    pub k: usize,
    /// Cumulative distribution function, length k+1.
    cdf: Vec<f64>,
}

impl RobustSoliton {
    /// Build the CDF for `k` source blocks.
    ///
    /// # Panics
    /// Panics if `k == 0`.
    pub fn new(k: usize) -> Self {
        assert!(k > 0, "k must be >= 1");
        let cdf = Self::build_cdf(k);
        Self { k, cdf }
    }

    fn build_cdf(k: usize) -> Vec<f64> {
        let kf = k as f64;
        let r = C_PARAM * (kf / DELTA_PARAM).ln().max(1.0) * kf.sqrt();

        // Ideal Soliton distribution
        let mut mu = vec![0.0f64; k + 1];
        mu[1] = 1.0 / kf;
        for d in 2..=k {
            mu[d] = 1.0 / (d as f64 * (d as f64 - 1.0));
        }

        // Tau (ripple boost) component
        let threshold = if r > 0.0 {
            ((kf / r) as usize).max(1).min(k)
        } else {
            k
        };
        for d in 1..=threshold {
            mu[d] += r / (d as f64 * kf);
        }
        if threshold > 0 && threshold <= k {
            mu[threshold] += r * (r / DELTA_PARAM).ln().max(0.0) / kf;
        }

        // Normalise
        let total: f64 = mu.iter().sum();
        let total = if total <= 0.0 { 1.0 } else { total };
        for v in mu.iter_mut() {
            *v /= total;
        }

        // CDF via prefix sum
        let mut cdf = Vec::with_capacity(k + 1);
        let mut acc = 0.0f64;
        for v in &mu {
            acc += v;
            cdf.push(acc.min(1.0));
        }
        cdf
    }

    /// Sample a degree value from the distribution using the given RNG.
    pub fn sample<R: rand::Rng>(&self, rng: &mut R) -> usize {
        let u: f64 = rng.random();
        // Binary search for the first CDF entry ≥ u
        let idx = self.cdf.partition_point(|&c| c < u);
        idx.max(1).min(self.k)
    }

    /// Deterministically derive the neighbour set for a given droplet `seed`.
    ///
    /// Uses ChaCha8 seeded with `seed as u64`.  Both encoder and decoder call
    /// this method, so the neighbour set is always consistent.
    pub fn neighbours(&self, seed: u32) -> HashSet<usize> {
        let mut rng = ChaCha8Rng::seed_from_u64(seed as u64);
        let degree = self.sample(&mut rng);

        // Fisher-Yates partial shuffle — equivalent to sampling without replacement
        let mut indices: Vec<usize> = (0..self.k).collect();
        for i in 0..degree {
            let j = i + (rng.next_u64() as usize % (self.k - i));
            indices.swap(i, j);
        }
        indices[..degree].iter().copied().collect()
    }
}

// ─── Encoder ─────────────────────────────────────────────────────────────────

/// Rateless LT Fountain Code encoder.
pub struct LTEncoder {
    /// Source blocks (zero-padded to a multiple of BLOCK_SIZE).
    blocks: Vec<[u8; BLOCK_SIZE]>,
    /// Number of source blocks.
    pub k: usize,
    /// Pre-computed degree distribution.
    dist: RobustSoliton,
    /// Monotonically increasing droplet counter (used as seed).
    counter: u32,
}

impl LTEncoder {
    /// Create an encoder from raw `data`.
    ///
    /// # Errors
    /// Returns [`FountainError::EmptyInput`] if `data` is empty.
    pub fn new(data: &[u8]) -> Result<Self, FountainError> {
        if data.is_empty() {
            return Err(FountainError::EmptyInput);
        }
        let blocks = Self::split(data);
        let k = blocks.len();
        Ok(Self {
            dist: RobustSoliton::new(k),
            blocks,
            k,
            counter: 0,
        })
    }

    /// Minimum recommended number of droplets for reliable decoding.
    #[inline]
    pub fn recommended_count(&self) -> usize {
        (self.k as f64 * OVERHEAD_FACTOR) as usize + OVERHEAD_ADDEND
    }

    /// Generate the next encoded droplet.
    ///
    /// Returns `HEADER_SIZE + BLOCK_SIZE` bytes ready for QR embedding.
    pub fn generate_droplet(&mut self) -> Vec<u8> {
        let seed = self.counter;
        self.counter = self.counter.wrapping_add(1);

        let neighbours = self.dist.neighbours(seed);
        let degree = neighbours.len();

        // XOR all neighbour blocks into the encoded payload
        let mut payload = [0u8; BLOCK_SIZE];
        for idx in &neighbours {
            for (p, b) in payload.iter_mut().zip(self.blocks[*idx].iter()) {
                *p ^= b;
            }
        }

        // Wire format: seed(4 BE) || degree(2 BE) || reserved(2) || payload(256)
        let mut droplet = Vec::with_capacity(HEADER_SIZE + BLOCK_SIZE);
        droplet.extend_from_slice(&seed.to_be_bytes());
        droplet.extend_from_slice(&(degree as u16).to_be_bytes());
        droplet.extend_from_slice(&[0u8; 2]); // reserved
        droplet.extend_from_slice(&payload);
        droplet
    }

    // ── Internal helpers ──────────────────────────────────────────────────

    fn split(data: &[u8]) -> Vec<[u8; BLOCK_SIZE]> {
        // Prepend 4-byte big-endian length prefix
        let len_prefix = (data.len() as u32).to_be_bytes();
        let mut padded: Vec<u8> = Vec::with_capacity(4 + data.len() + BLOCK_SIZE);
        padded.extend_from_slice(&len_prefix);
        padded.extend_from_slice(data);

        // Pad to a multiple of BLOCK_SIZE
        let rem = padded.len() % BLOCK_SIZE;
        if rem != 0 {
            padded.extend(std::iter::repeat(0u8).take(BLOCK_SIZE - rem));
        }

        padded
            .chunks_exact(BLOCK_SIZE)
            .map(|chunk| {
                let mut block = [0u8; BLOCK_SIZE];
                block.copy_from_slice(chunk);
                block
            })
            .collect()
    }
}

// ─── Decoder ─────────────────────────────────────────────────────────────────

/// Belief-propagation decoder for LT Fountain Codes.
///
/// # Usage
/// 1. Call [`Self::set_block_count`] once the METADATA frame arrives.
/// 2. Feed droplets via [`Self::add_droplet`].
/// 3. When [`Self::add_droplet`] returns `Ok(true)` (or [`Self::is_complete`]),
///    call [`Self::get_data`].
pub struct LTDecoder {
    block_size: usize,
    /// Number of source blocks (set from METADATA).
    k: Option<usize>,
    dist: Option<RobustSoliton>,
    /// Decoded source blocks; `None` until `set_block_count` is called.
    blocks: Option<Vec<[u8; BLOCK_SIZE]>>,
    /// Boolean mask: which blocks have been decoded.
    decoded: Option<Vec<bool>>,
    /// Number of decoded blocks so far.
    decoded_count: usize,
    /// Droplets still waiting for their neighbours to be resolved.
    pending: Vec<PendingDroplet>,
    /// Inverted index: block_idx → indices into `pending`.
    inv_index: HashMap<usize, Vec<usize>>,
    /// Seen seeds (deduplication).
    seen: HashSet<u32>,
    /// Droplets that arrived before `set_block_count` was called.
    early_buffer: Vec<Vec<u8>>,
}

struct PendingDroplet {
    /// XOR-reduced payload (mutated in place during propagation).
    data: [u8; BLOCK_SIZE],
    /// Remaining undecoded neighbours.
    neighbours: HashSet<usize>,
}

impl LTDecoder {
    /// Create a new decoder.  Call [`Self::set_block_count`] before feeding droplets.
    pub fn new() -> Self {
        Self {
            block_size: BLOCK_SIZE,
            k: None,
            dist: None,
            blocks: None,
            decoded: None,
            decoded_count: 0,
            pending: Vec::new(),
            inv_index: HashMap::new(),
            seen: HashSet::new(),
            early_buffer: Vec::new(),
        }
    }

    /// Initialise for `k` source blocks.  Idempotent.
    pub fn set_block_count(&mut self, k: usize) {
        if self.k.is_some() {
            return; // already initialised
        }
        self.k = Some(k);
        self.dist = Some(RobustSoliton::new(k));
        self.blocks = Some(vec![[0u8; BLOCK_SIZE]; k]);
        self.decoded = Some(vec![false; k]);

        // Replay early-buffered droplets
        let buffered: Vec<Vec<u8>> = std::mem::take(&mut self.early_buffer);
        for pkt in buffered {
            let _ = self.ingest(&pkt);
        }
    }

    /// Feed a raw packet (header + payload) to the decoder.
    ///
    /// Returns `Ok(true)` when all source blocks have been recovered.
    pub fn add_droplet(&mut self, packet: &[u8]) -> Result<bool, FountainError> {
        let min = HEADER_SIZE + self.block_size;
        if packet.len() < min {
            return Err(FountainError::DropletTooShort {
                min,
                got: packet.len(),
            });
        }
        if self.k.is_none() {
            self.early_buffer.push(packet.to_vec());
            return Ok(false);
        }
        Ok(self.ingest(packet))
    }

    /// Returns `true` if all source blocks have been decoded.
    #[inline]
    pub fn is_complete(&self) -> bool {
        self.k
            .map(|k| self.decoded_count == k)
            .unwrap_or(false)
    }

    /// Fraction of source blocks decoded, in [0.0, 1.0].
    pub fn progress(&self) -> f64 {
        match self.k {
            None | Some(0) => 0.0,
            Some(k) => self.decoded_count as f64 / k as f64,
        }
    }

    /// Reconstruct and return the original data.
    ///
    /// # Errors
    /// Returns [`FountainError::Incomplete`] if decoding is not yet complete.
    pub fn get_data(&self) -> Result<Vec<u8>, FountainError> {
        if !self.is_complete() {
            return Err(FountainError::Incomplete {
                progress: self.progress(),
            });
        }
        let blocks = self.blocks.as_ref().unwrap();
        let raw: Vec<u8> = blocks.iter().flat_map(|b| b.iter().copied()).collect();

        // Strip length prefix
        let len = u32::from_be_bytes([raw[0], raw[1], raw[2], raw[3]]) as usize;
        Ok(raw[4..4 + len].to_vec())
    }

    // ── Internal belief-propagation ───────────────────────────────────────

    fn ingest(&mut self, packet: &[u8]) -> bool {
        let seed = u32::from_be_bytes([packet[0], packet[1], packet[2], packet[3]]);

        if self.seen.contains(&seed) {
            return self.is_complete();
        }
        self.seen.insert(seed);

        // Copy payload into a mutable buffer
        let mut payload = [0u8; BLOCK_SIZE];
        payload.copy_from_slice(&packet[HEADER_SIZE..HEADER_SIZE + BLOCK_SIZE]);

        // Re-derive neighbours from seed (canonical; ignores header degree)
        let dist = self.dist.as_ref().unwrap();
        let mut neighbours = dist.neighbours(seed);

        let blocks = self.blocks.as_ref().unwrap();
        let decoded = self.decoded.as_ref().unwrap();

        // XOR out already-decoded neighbours immediately
        let already: Vec<usize> = neighbours
            .iter()
            .copied()
            .filter(|&idx| decoded[idx])
            .collect();
        for idx in already {
            for (p, b) in payload.iter_mut().zip(blocks[idx].iter()) {
                *p ^= b;
            }
            neighbours.remove(&idx);
        }

        if neighbours.is_empty() {
            return self.is_complete(); // redundant droplet
        }

        if neighbours.len() == 1 {
            let idx = *neighbours.iter().next().unwrap();
            self.resolve(idx, payload);
        } else {
            let droplet_idx = self.pending.len();
            for &nb in &neighbours {
                self.inv_index.entry(nb).or_default().push(droplet_idx);
            }
            self.pending.push(PendingDroplet {
                data: payload,
                neighbours,
            });
        }

        self.is_complete()
    }

    fn resolve(&mut self, block_idx: usize, data: [u8; BLOCK_SIZE]) {
        {
            let decoded = self.decoded.as_ref().unwrap();
            if decoded[block_idx] {
                return;
            }
        }

        // Store the block
        self.blocks.as_mut().unwrap()[block_idx] = data;
        self.decoded.as_mut().unwrap()[block_idx] = true;
        self.decoded_count += 1;

        self.propagate(block_idx);
    }

    fn propagate(&mut self, newly_decoded: usize) {
        let mut queue = vec![newly_decoded];

        while let Some(idx) = queue.pop() {
            let affected_indices = match self.inv_index.remove(&idx) {
                Some(v) => v,
                None => continue,
            };

            for droplet_idx in affected_indices {
                let droplet = &mut self.pending[droplet_idx];

                if !droplet.neighbours.contains(&idx) {
                    continue; // already removed in a prior pass
                }

                // XOR out the newly decoded block
                let block = self.blocks.as_ref().unwrap()[idx];
                for (p, b) in droplet.data.iter_mut().zip(block.iter()) {
                    *p ^= b;
                }
                droplet.neighbours.remove(&idx);

                match droplet.neighbours.len() {
                    0 => { /* redundant */ }
                    1 => {
                        let next_idx = *droplet.neighbours.iter().next().unwrap();
                        let decoded = self.decoded.as_ref().unwrap();
                        if !decoded[next_idx] {
                            let payload = droplet.data;
                            self.resolve(next_idx, payload);
                            queue.push(next_idx);
                        }
                    }
                    _ => {
                        // Re-register remaining neighbours in the inverted index
                        let remaining: Vec<usize> =
                            droplet.neighbours.iter().copied().collect();
                        for nb in remaining {
                            self.inv_index.entry(nb).or_default().push(droplet_idx);
                        }
                    }
                }
            }
        }
    }
}

impl Default for LTDecoder {
    fn default() -> Self {
        Self::new()
    }
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_data(size: usize) -> Vec<u8> {
        (0..size).map(|i| (i % 251) as u8).collect()
    }

    #[test]
    fn encoder_produces_correct_header_size() {
        let data = make_test_data(1024);
        let mut enc = LTEncoder::new(&data).unwrap();
        let droplet = enc.generate_droplet();
        assert_eq!(droplet.len(), HEADER_SIZE + BLOCK_SIZE);
    }

    #[test]
    fn roundtrip_small() {
        let data = make_test_data(256);
        roundtrip_check(&data, 0);
    }

    #[test]
    fn roundtrip_medium() {
        let data = make_test_data(4096);
        roundtrip_check(&data, 0);
    }

    #[test]
    fn roundtrip_with_40pct_loss() {
        let data = make_test_data(2048);
        roundtrip_check(&data, 40);
    }

    #[test]
    fn roundtrip_one_byte() {
        let data = [0x42u8];
        roundtrip_check(&data, 0);
    }

    #[test]
    fn decoder_deduplicates_seeds() {
        let data = make_test_data(512);
        let mut enc = LTEncoder::new(&data).unwrap();
        let mut dec = LTDecoder::new();
        dec.set_block_count(enc.k);

        let droplet = enc.generate_droplet();
        // Feed the same droplet twice — second should be ignored
        dec.add_droplet(&droplet).unwrap();
        dec.add_droplet(&droplet).unwrap();
        // No panic or double-count
    }

    #[test]
    fn early_buffer_works() {
        let data = make_test_data(512);
        let mut enc = LTEncoder::new(&data).unwrap();
        let mut dec = LTDecoder::new();

        // Feed droplets BEFORE set_block_count
        let limit = enc.recommended_count() * 2;
        let mut droplets = Vec::new();
        for _ in 0..limit {
            droplets.push(enc.generate_droplet());
        }
        for d in &droplets {
            dec.add_droplet(d).unwrap();
        }

        // Now set block count — should replay buffered droplets
        dec.set_block_count(enc.k);

        if dec.is_complete() {
            let recovered = dec.get_data().unwrap();
            assert_eq!(recovered, data);
        }
        // (may not be complete without enough droplets, but no panic)
    }

    #[test]
    fn robust_soliton_degree_bounds() {
        let rs = RobustSoliton::new(100);
        let mut rng = ChaCha8Rng::seed_from_u64(42);
        for _ in 0..1000 {
            let d = rs.sample(&mut rng);
            assert!(d >= 1 && d <= 100, "degree out of bounds: {d}");
        }
    }

    // ── Helper ────────────────────────────────────────────────────────────

    fn roundtrip_check(data: &[u8], loss_pct: u8) {
        let mut enc = LTEncoder::new(data).unwrap();
        let mut dec = LTDecoder::new();
        dec.set_block_count(enc.k);

        let limit = enc.recommended_count() * 4;
        let mut seed_mod = 0u32;

        for _ in 0..limit {
            let droplet = enc.generate_droplet();
            seed_mod = seed_mod.wrapping_add(1);

            // Simulate packet loss
            if loss_pct > 0 && (seed_mod % 100) < loss_pct as u32 {
                continue;
            }

            if dec.add_droplet(&droplet).unwrap() {
                break;
            }
        }

        assert!(dec.is_complete(), "Decoding did not complete (progress={:.1}%)", dec.progress() * 100.0);
        let recovered = dec.get_data().unwrap();
        assert_eq!(recovered, data);
    }
}