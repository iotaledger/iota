// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Libtorch (tch) port of the hotness-aware gas predictor.
//!
//! Parity with Python model:
//! - LSTM encoder (hidden=64) + LayerNorm
//! - Attention-based DeepSets aggregator
//! - Head: Linear -> GELU -> Linear to Q (quantiles)
//! - Quantile (pinball) loss
//! - Same TAUS, LR, WD, grad-clip
//!
//! Public API to use from CongestionTracker:
//!   let mut learner = GasLearner::new(tch::Device::Cpu)?;
//!   learner.warmup();
//!   let (pred, attn) = learner.predict(&seqs, h_anchor);
//!   let avg_loss = learner.train_step(batch_seqs, batch_h, batch_ylog)?;

use tch::{
    Device, IndexOp, Tensor,
    kind::Kind,
    nn::{self, Module, OptimizerConfig, RNN},
};

// ==============================
// Config (identical to Python)
// ==============================
pub const T: i64 = 10; // window length per object
pub const MIN_GAS_FLOOR: f32 = 1000.0; // used outside model
pub const TAUS: [f32; 3] = [0.5, 0.8, 0.9];
pub const DEFAULT_TAU: f32 = 0.8;

pub const LR: f64 = 1e-3;
pub const WEIGHT_DECAY: f64 = 1e-4;
pub const MAX_GRAD_NORM: f64 = 2.0;

// Feature count (matches FEAT_KEYS in Python)
// Input feature count: base 9 + EMA(low/high) + time-since + padded-flag = 13
pub const F: i64 = 13;
pub const EMBED_DIM: i64 = 64;

// ==============================
// Helpers
// ==============================

/// Index of the quantile closest to `tau`.
pub fn tau_index(tau: f32) -> usize {
    let mut best_i = 0usize;
    let mut best_d = (TAUS[0] - tau).abs();
    for (i, &t) in TAUS.iter().enumerate().skip(1) {
        let d = (t - tau).abs();
        if d < best_d {
            best_d = d;
            best_i = i;
        }
    }
    best_i
}

// ==============================
// TemporalEncoder: LSTM + LayerNorm
// ==============================
pub struct TemporalEncoder {
    lstm: nn::LSTM,
    norm: nn::LayerNorm,
}

impl TemporalEncoder {
    pub fn new(vs: &nn::Path, input_dim: i64, hidden_dim: i64) -> Self {
        let lstm_cfg = nn::RNNConfig {
            batch_first: true,
            ..Default::default()
        };
        let lstm = nn::lstm(vs / "lstm", input_dim, hidden_dim, lstm_cfg);
        let norm = nn::layer_norm(vs / "ln", vec![hidden_dim], Default::default());
        Self { lstm, norm }
    }

    /// x: (B, T, F) -> emb: (B, H)
    pub fn forward(&self, x: &Tensor, train: bool) -> Tensor {
        let (_out, state) = self.lstm.seq(x); // state: (layers, B, H)
        let h = state.h().squeeze_dim(0); // (B, H)
        let h = h.apply(&self.norm); // (B, H)
        if train { h.dropout(0.1, true) } else { h }
    }
}

// ==============================
// SetAggregator: attention + MLP head
// head input: context(H) + mean(H) + max(H) + log(count)(1) + h_anchor(1) => 3H
// + 2 ==============================
pub struct SetAggregator {
    score: nn::Sequential,
    head: nn::Sequential,
    _taus: Vec<f32>,
}

impl SetAggregator {
    pub fn new(vs: &nn::Path, embed_dim: i64, taus: &[f32]) -> Self {
        let score = nn::seq()
            .add(nn::linear(
                vs / "s1",
                embed_dim,
                embed_dim / 2,
                Default::default(),
            ))
            .add_fn(|x| x.tanh())
            .add(nn::linear(vs / "s2", embed_dim / 2, 1, Default::default()));

        let head_in = 3 * embed_dim + 2;
        let head = nn::seq()
            .add(nn::linear(vs / "h1", head_in, 128, Default::default()))
            .add_fn(|x| x.gelu("none"))
            .add(nn::linear(
                vs / "h2",
                128,
                taus.len() as i64,
                Default::default(),
            ));

        Self {
            score,
            head,
            _taus: taus.to_vec(),
        }
    }

    /// E: (N, H), h_anchor: scalar tensor -> (pred_log_delta(Q,), attn(N,))
    pub fn forward(&self, e: &Tensor, h_anchor: &Tensor, train: bool) -> (Tensor, Tensor) {
        if e.size().is_empty() || e.size()[0] == 0 {
            let q = TAUS.len() as i64;
            return (
                Tensor::zeros([q], (Kind::Float, e.device())),
                Tensor::zeros([0], (Kind::Float, e.device())),
            );
        }
        // attention over objects
        let scores = self.score.forward(e).squeeze_dim(-1); // (N,)
        let attn = scores.softmax(0, Kind::Float); // (N,)
        let reduce_dim = [0i64];
        let context = (attn.unsqueeze(-1) * e).sum_dim_intlist(&reduce_dim[..], false, Kind::Float); // (H,)
        let mean = e.mean_dim(&reduce_dim[..], false, Kind::Float); // (H,)
        let max = e.max_dim(0, false).0; // (H,)

        let n = e.size()[0] as f32;
        let count_feat = Tensor::from(n.ln_1p()).to_device(e.device());

        let head_in = Tensor::cat(
            &[
                context,                                  // H
                mean,                                     // H
                max,                                      // H
                count_feat.view([1]),                     // 1
                h_anchor.view([1]).to_device(e.device()), // 1
            ],
            0,
        ); // (3H+2,)

        let head_in = if train {
            head_in.dropout(0.1, true)
        } else {
            head_in
        };
        let pred = self.head.forward(&head_in.unsqueeze(0)).squeeze_dim(0); // (Q,)
        (pred, attn)
    }
}

// ==============================
// PriceModel: encoder + aggregator
// ==============================
pub struct PriceModel {
    encoder: TemporalEncoder,
    agg: SetAggregator,
}

impl PriceModel {
    pub fn new(vs: &nn::Path, input_dim: i64, embed_dim: i64, taus: &[f32]) -> Self {
        let encoder = TemporalEncoder::new(&(vs / "temporal"), input_dim, embed_dim);
        let agg = SetAggregator::new(&(vs / "agg"), embed_dim, taus);
        Self { encoder, agg }
    }

    /// seqs: &[Tensor] each (T,F); h_anchor: max hotness_over_ref across
    /// objects returns (pred_log_deltas (Q,), attn (N,))
    pub fn forward_tx(&self, seqs: &[Tensor], h_anchor: f32, train: bool) -> (Tensor, Tensor) {
        if seqs.is_empty() {
            let q = TAUS.len() as i64;
            return (
                Tensor::zeros([q], (Kind::Float, Device::Cpu)),
                Tensor::zeros([0], (Kind::Float, Device::Cpu)),
            );
        }
        let x = Tensor::stack(seqs, 0); // (N, T, F)
        let e = self.encoder.forward(&x, train); // (N, H)
        let h_anchor_t = Tensor::from(h_anchor); // scalar
        self.agg.forward(&e, &h_anchor_t, train)
    }
}

// ==============================
// Quantile (pinball) loss
// pred: (B,Q), target: (B,)
// ==============================
pub fn pinball_loss(pred: &Tensor, target: &Tensor, taus: &[f32]) -> Tensor {
    let diff = target.unsqueeze(1) - pred; // (B,Q)
    let b = diff.size()[0];
    let q = diff.size()[1];
    let mut per_q = Vec::with_capacity(q as usize);
    for j in 0..q {
        let e_j = diff.i((0..b, j));
        // max(tau*e, (tau-1)*e)
        let tau = f64::from(taus[j as usize]);
        let left = &e_j * tau;
        let right = &e_j * (tau - 1.0);
        let l = left.maximum(&right).mean(Kind::Float);
        per_q.push(l);
    }
    Tensor::stack(&per_q, 0).mean(Kind::Float)
}

// ==============================
// Learner wrapper (optimizer, training step, warmup, predict)
// ==============================
pub struct GasLearner {
    pub vs: nn::VarStore,
    pub model: PriceModel,
    pub opt: nn::Optimizer,
}

impl GasLearner {
    pub fn new(device: Device) -> tch::Result<Self> {
        let vs = nn::VarStore::new(device);
        let root = &vs.root();
        let model = PriceModel::new(root, F, EMBED_DIM, &TAUS);
        let opt = nn::Adam {
            wd: WEIGHT_DECAY,
            ..Default::default()
        }
        .build(&vs, LR)?;
        Ok(Self { vs, model, opt })
    }

    /// One-time warm-up to avoid first-call stalls (parity with Python
    /// startup).
    pub fn warmup(&self) {
        tch::no_grad(|| {
            let dummy = Tensor::zeros([1, T, F], (Kind::Float, self.vs.device()));
            let (_pred, _attn) = self
                .model
                .forward_tx(&[dummy.squeeze_dim(0)], 0.0f32, false);
        });
    }

    /// Inference for one tx: returns (pred_log_deltas(Q,), attn(N,))
    pub fn predict(&self, seqs: &[Tensor], h_anchor: f32) -> (Tensor, Tensor) {
        tch::no_grad(|| self.model.forward_tx(seqs, h_anchor, false))
    }

    /// Train on a small in-memory batch (parity with _train_on_batch):
    /// - xs: Vec of tx histories; each tx is Vec<(T,F) Tensor> for its objects
    /// - h_anchors: per-tx max hotness_over_ref
    /// - y_logs: per-tx target log-delta
    ///
    /// Returns average loss.
    pub fn train_step(
        &mut self,
        xs: Vec<Vec<Tensor>>,
        h_anchors: Vec<f32>,
        y_logs: Vec<f32>,
    ) -> tch::Result<f32> {
        assert_eq!(xs.len(), h_anchors.len());
        assert_eq!(xs.len(), y_logs.len());

        let mut losses: Vec<Tensor> = Vec::with_capacity(xs.len());

        for (seqs, (h, y)) in xs
            .into_iter()
            .zip(h_anchors.into_iter().zip(y_logs.into_iter()))
        {
            let (pred, _attn) = self.model.forward_tx(&seqs, h, true); // (Q,)
            let pred_b = pred.unsqueeze(0); // (1,Q)
            let target = Tensor::from(y).to_device(self.vs.device()).unsqueeze(0); // (1,)
            let loss = pinball_loss(&pred_b, &target, &TAUS);
            losses.push(loss);
        }

        let total = Tensor::stack(&losses, 0).mean(Kind::Float);
        self.opt.zero_grad();
        total.backward();
        self.opt.clip_grad_norm(MAX_GRAD_NORM);
        self.opt.step();

        Ok(total.double_value(&[]) as f32)
    }
}

// ==============================
// Optional: convenience to build a (T,F) tensor from a flat slice
// ==============================
pub fn tensor_from_rows_txf(rows: &[[f32; F as usize]]) -> Tensor {
    let flat: Vec<f32> = rows.iter().flat_map(|r| r.iter().copied()).collect();
    Tensor::from_slice(&flat).view([rows.len() as i64, F])
}
