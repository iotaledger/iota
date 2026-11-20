// TEMP benchmark: the test transactions of the PTB limits benchmark. This file
// has two byte-for-byte copies, which network-benchmark's
// `scripts/ptb-limits/check_same.sh` compares:
//   - iota: crates/iota-e2e-tests/tests/ptb_limits/ptb_limits_tx.rs (setup 1
//     test)
//   - network-benchmark: crates/iota-benchmark/src/workloads/ptb_limits_tx.rs
//     (the `ptb-limits` workload of `stress`, setup 2)
// The including module provides the imported names. Not for commit.

use super::{Argument, CallArg, Identifier, ObjectId, ObjectReference, ProgrammableTransactionBuilder, SharedObjectReference, Version};

/// Dynamic field reads per `df_read` call of the `combo` kernel.
const COMBO_READS_PER_CALL: u64 = 490;
/// Dynamic fields added per setup transaction.
const FILL_PER_TX: u64 = 400;
/// Owned objects created per setup transaction.
const CREATE_PER_TX: u64 = 1000;

/// What the test transaction does and how much of it, from the `STRESS_*`
/// environment variables that `break_ladder.py` sets for each step.
#[derive(Clone, Debug)]
pub struct Spec {
    /// The function of the `kernels` module, or `noop_upgraded` / `noop_big`
    /// (calls into the big package), `inputs` (owned objects as inputs) or
    /// `combo` (calls into the upgraded package, dynamic field reads, BLS).
    pub kernel: String,
    pub commands: usize,
    pub n: u64,
    pub bytes: usize,
    pub budget: u64,
    /// `combo`: the dynamic fields read.
    pub df_total: u64,
    /// `df_read`: the reads, split over the commands.
    pub df_reads: u64,
    /// `inputs`: the owned objects the setup creates.
    pub inputs_total: u64,
    /// `sink`: the size of the last argument.
    pub last_bytes: Option<usize>,
}

impl Spec {
    pub fn from_env() -> Self {
        let var = |name: &str| std::env::var(name).ok().and_then(|v| v.parse::<u64>().ok());
        let commands = var("STRESS_COMMANDS").unwrap_or(1) as usize;
        let n = var("STRESS_N").unwrap_or(1);
        Self {
            kernel: std::env::var("STRESS_KERNEL").expect("STRESS_KERNEL is not set"),
            commands,
            n,
            bytes: var("STRESS_BYTES").unwrap_or(0) as usize,
            budget: var("LIMITS_GAS_BUDGET").unwrap_or(50_000_000_000),
            df_total: var("STRESS_DF").unwrap_or(0),
            df_reads: var("STRESS_TOTAL").unwrap_or(commands as u64 * n),
            inputs_total: var("STRESS_TOTAL").unwrap_or(n),
            last_bytes: var("STRESS_LAST_BYTES").map(|v| v as usize),
        }
    }

    /// Whether the setup publishes the big package, and upgrades it once.
    pub fn big_package(&self) -> Option<bool> {
        match self.kernel.as_str() {
            "noop_upgraded" | "combo" => Some(true),
            "noop_big" => Some(false),
            _ => None,
        }
    }

    /// How many dynamic fields the setup adds to the parent object, if any.
    /// For `df_read`, a few more than the transaction reads, so that a read of
    /// one more field (the check that the limit was reached) finds one.
    pub fn parent_fields(&self) -> Option<u64> {
        match self.kernel.as_str() {
            "combo" => Some(self.df_total),
            "df_read" => Some(self.df_reads + 64),
            _ => None,
        }
    }

    /// How many owned objects the setup creates, if any.
    pub fn owned_objects(&self) -> Option<u64> {
        (self.kernel == "inputs").then_some(self.inputs_total)
    }

    /// The argument bytes of the kernels that take a byte vector.
    pub fn data(&self) -> Vec<u8> {
        (0..self.bytes).map(|i| (i % 251) as u8).collect()
    }
}

/// What the setup created.
#[derive(Clone, Debug)]
pub struct Objects {
    /// The `stress` package (`kernels` module).
    pub stress: ObjectId,
    /// The big package, after the upgrade when there is one.
    pub big: Option<ObjectId>,
    /// The parent of the dynamic fields, with its initial shared version.
    pub parent: Option<(ObjectId, Version)>,
    /// The owned objects for `inputs`, at their current versions.
    pub inputs: Vec<ObjectReference>,
}

fn call(b: &mut ProgrammableTransactionBuilder, package: ObjectId, module: &str, function: &str, args: Vec<Argument>) {
    b.programmable_move_call(package, Identifier::new(module).unwrap(), Identifier::new(function).unwrap(), vec![], args);
}

/// Setup: create the parent object of the dynamic fields.
pub fn create_parent(stress: ObjectId) -> ProgrammableTransactionBuilder {
    let mut b = ProgrammableTransactionBuilder::new();
    call(&mut b, stress, "kernels", "create_parent", vec![]);
    b
}

/// Setup: the transactions that add `total` dynamic fields to the parent.
/// `df::add` looks the field up first, which counts toward the dynamic field
/// limit, so they are added a few hundred at a time.
pub fn fill_parent(stress: ObjectId, parent: (ObjectId, Version), total: u64) -> Vec<ProgrammableTransactionBuilder> {
    let mut txs = Vec::new();
    let mut start = 0;
    while start < total {
        let count = (total - start).min(FILL_PER_TX);
        let mut b = ProgrammableTransactionBuilder::new();
        let p = b.obj(CallArg::Shared(SharedObjectReference::new(parent.0, parent.1, true))).unwrap();
        let (s, c) = (b.pure(start).unwrap(), b.pure(count).unwrap());
        call(&mut b, stress, "kernels", "fill", vec![p, s, c]);
        txs.push(b);
        start += count;
    }
    txs
}

/// Setup: the transactions that create `total` owned objects.
pub fn create_objects(stress: ObjectId, total: u64) -> Vec<ProgrammableTransactionBuilder> {
    let mut txs = Vec::new();
    let mut start = 0;
    while start < total {
        let count = (total - start).min(CREATE_PER_TX);
        let mut b = ProgrammableTransactionBuilder::new();
        let c = b.pure(count).unwrap();
        call(&mut b, stress, "kernels", "create_objects", vec![c]);
        txs.push(b);
        start += count;
    }
    txs
}

/// The test transaction.
pub fn test_transaction(spec: &Spec, objects: &Objects, data: &[u8]) -> ProgrammableTransactionBuilder {
    let mut b = ProgrammableTransactionBuilder::new();
    let (n, commands) = (spec.n, spec.commands);
    if spec.kernel == "combo" {
        let df_calls = spec.df_total.div_ceil(COMBO_READS_PER_CALL);
        let (id, version) = objects.parent.unwrap();
        let p = b.obj(CallArg::Shared(SharedObjectReference::new(id, version, false))).unwrap();
        for _ in 0..(commands as u64 - df_calls - 1) {
            call(&mut b, objects.big.unwrap(), "b_entry", "noop", vec![]);
        }
        for i in 0..df_calls {
            let args = vec![
                p,
                b.pure(i * COMBO_READS_PER_CALL).unwrap(),
                b.pure((spec.df_total - i * COMBO_READS_PER_CALL).min(COMBO_READS_PER_CALL)).unwrap(),
            ];
            call(&mut b, objects.stress, "kernels", "df_read", args);
        }
        let rounds = b.pure(n).unwrap();
        call(&mut b, objects.stress, "kernels", "bls_loop", vec![rounds]);
        return b;
    }
    let (package, module, function) = match spec.kernel.as_str() {
        "noop_upgraded" | "noop_big" => (objects.big.unwrap(), "b_entry", "noop"),
        "inputs" => (objects.stress, "kernels", "noop"),
        kernel => (objects.stress, "kernels", kernel),
    };
    if spec.kernel == "inputs" {
        for input in objects.inputs.iter().take(n as usize) {
            b.obj(CallArg::ImmutableOrOwned(*input)).unwrap();
        }
    }
    for c in 0..commands {
        let args = match spec.kernel.as_str() {
            "noop" | "noop_upgraded" | "noop_big" | "inputs" => vec![],
            "u256_loop" | "ed25519_loop" | "k1_recover_loop" | "k1_verify_loop" | "bls_loop" | "ecvrf_loop"
            | "g1_mul_loop" => vec![b.pure(n).unwrap()],
            "bcs_loop" | "emit_big" | "create_big" => vec![b.pure(spec.bytes as u64).unwrap(), b.pure(n).unwrap()],
            // A distinct blob per command, so the builder keeps each as its own input.
            "sink" => {
                let mut blob = data.to_vec();
                if let Some(last) = spec.last_bytes.filter(|_| c == commands - 1) {
                    blob.resize(last.max(8), 0);
                }
                blob[..8].copy_from_slice(&(c as u64).to_le_bytes());
                vec![b.pure(blob).unwrap()]
            }
            "df_read" => {
                let (id, version) = objects.parent.unwrap();
                let p = b.obj(CallArg::Shared(SharedObjectReference::new(id, version, false))).unwrap();
                vec![p, b.pure(c as u64 * n).unwrap(), b.pure(n.min(spec.df_reads.saturating_sub(c as u64 * n))).unwrap()]
            }
            "create_objects" => vec![b.pure(n).unwrap()],
            // Kernels that take a byte vector and a count.
            _ => vec![b.pure(data.to_vec()).unwrap(), b.pure(n).unwrap()],
        };
        call(&mut b, package, module, function, args);
    }
    b
}
