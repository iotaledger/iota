// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::time::Duration;

#[path = "svg_template.rs"]
mod svg_template;

#[derive(Clone, Copy, Debug)]
struct Node {
    title: &'static str,
    samples: usize,
    dur: Duration,
    percent: f64,
    x: f64,
    y: usize,
    width: f64,
    height: usize,
    rgb: (u8, u8, u8),
}

#[derive(Clone, Copy, Debug)]
pub struct Config {
    /// Desired resolution in nanoseconds per pixel.
    pub resolution_nanos_per_px: Option<u128>,
    /// Desired width of the SVG in pixels.
    pub width: Option<usize>,
    /// Desired aspect ratio (width, height) of the SVG.
    pub aspect_ratio: Option<(usize, usize)>,
    /// Seed value for random color generation to ensure reproducible flamegraph
    /// colors.
    pub seed: u64,
}
impl Default for Config {
    fn default() -> Config {
        Config {
            resolution_nanos_per_px: None,
            width: Some(1920),
            aspect_ratio: None,
            seed: 1,
        }
    }
}
#[derive(Clone, Debug)]
pub struct Svg {
    svg: String,
}
impl Svg {
    pub fn as_str(&self) -> &str {
        &self.svg
    }
    pub fn into_string(self) -> String {
        self.svg
    }
}

pub trait Renderer {
    fn raw_svg(&self, raw: &mut Raw, indent: bool);
    fn render_svg(&self, caption: &str, config: &Config) -> Svg {
        let mut raw = Raw::default();
        self.raw_svg(&mut raw, false);
        raw.render(caption, config)
    }
}

use super::{
    callgraph::{CallGraph, Frame, NodeId},
    flame::{Flames, FrameLabel, Metadata},
    metric::{FlameMetric, MergeMetrics, SpanMetrics},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RawNode {
    label: FrameLabel,
    samples: usize,
    start: Duration,
    total: Duration,
}
impl RawNode {
    fn into_svg<R: rand::Rng>(
        self,
        overall: Duration,
        x_scale: f64,
        y: usize,
        rng: &mut R,
    ) -> Node {
        let RawNode {
            label,
            samples,
            start,
            total,
        } = self;
        let rgb = random_rgb(rng);
        Node {
            title: label.name,
            samples,
            dur: total,
            percent: total.as_nanos() as f64 * 100.0 / overall.as_nanos() as f64,
            x: start.as_nanos() as f64 * x_scale + 10.0,
            y,
            width: total.as_nanos() as f64 * x_scale,
            height: 15,
            rgb,
        }
    }
}
fn random_rgb<R: rand::Rng>(rng: &mut R) -> (u8, u8, u8) {
    let r = rng.gen_range(150..=255);
    let g = rng.gen_range(0..=100);
    let b = rng.gen_range(0..=100);
    (r, g, b)
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Raw {
    total: Duration,
    running: Vec<Vec<RawNode>>,
}
impl Raw {
    fn add_node(&mut self, frame: &Frame<FlameMetric>, start: Duration, level: usize) -> Duration {
        if self.running.len() <= level {
            self.running.resize(level + 1, Vec::new());
        }
        self.running[level].push(RawNode {
            label: frame.label,
            samples: frame.metrics.count.entered,
            start,
            total: frame.metrics.running.total,
        });
        start
    }
    fn render(self, caption: &str, config: &Config) -> Svg {
        let Raw { total, running } = self;
        let num_levels = running.len();
        // 33px margins and 16px row height are hardcoded
        let height = 33 + num_levels * 16 + 33;

        let (x_scale, width) = config
            .resolution_nanos_per_px // try resolution first
            .and_then(|r| (r > 0).then_some(r))
            .map(|r| (1.0 / r as f64, (total.as_nanos() / r) as usize + 20))
            .unwrap_or_else(|| {
                let w = config
                    .width // try width next
                    .unwrap_or_else(|| {
                        // finally try aspect ratio
                        let (w, h) = config.aspect_ratio.unwrap_or((16, 9));
                        height * w / h
                    })
                    .max(100); // minimum width
                // 10px margin on each side
                if total.is_zero() {
                    (1.0, w)
                } else {
                    ((w - 20) as f64 / total.as_nanos() as f64, w)
                }
            });

        use rand::SeedableRng as _;
        let mut rng = rand::rngs::SmallRng::seed_from_u64(config.seed);

        let nodes = running
            .into_iter()
            .enumerate()
            .flat_map(|(i, row)| row.into_iter().map(move |raw| (raw, i)))
            .map(|(raw, level)| {
                let y = (num_levels - 1 - level) * 16 + 33;
                raw.into_svg(total, x_scale, y, &mut rng)
            });

        render(caption, width, height, nodes)
    }
}

impl Renderer for CallGraph<FlameMetric> {
    fn raw_svg(&self, raw: &mut Raw, indent: bool) {
        if !self.graph.is_empty() {
            let start = raw.total;
            let root_metrics = self.graph[NodeId::default()].value.metrics;
            raw.total += root_metrics.running.total;

            self.graph.dfs_fold2(
                raw,
                || start,
                |svg_raw, start, node_id, level| {
                    svg_raw.add_node(&self.graph[node_id].value, start, level + indent as usize)
                },
                |_, start, node_id| {
                    let metrics = self.graph[node_id].value.metrics;
                    *start + metrics.running.total
                },
            );
        }
    }
}

impl<S: Clone + Default + MergeMetrics + SpanMetrics> Flames<S>
where
    CallGraph<S>: Renderer,
{
    pub fn render_svg(
        &self,
        graph_id: &Metadata<'_>,
        running: bool,
        completed: bool,
        config: &Config,
    ) -> Option<Svg> {
        self.get_callgraph(graph_id, running, completed)
            .map(|callgraph| callgraph.render_svg(graph_id.caption, config))
    }
    pub fn render_combined_svg(
        &self,
        caption: &str,
        running: bool,
        completed: bool,
        config: &Config,
    ) -> Option<Svg> {
        let mut raw = self.get_callgraphs(running, completed).values().fold(
            Raw::default(),
            |mut raw, callgraph| {
                callgraph.raw_svg(&mut raw, true);
                raw
            },
        );
        if raw.total.is_zero() || raw.running.is_empty() {
            None
        } else {
            raw.running[0].push(RawNode {
                label: FrameLabel { name: "all" },
                samples: 1,
                start: Default::default(),
                total: raw.total,
            });
            Some(raw.render(caption, config))
        }
    }
}

fn render(caption: &str, width: usize, height: usize, nodes: impl Iterator<Item = Node>) -> Svg {
    let mut svg = String::new();
    svg.push_str(svg_template::XML_HEADER);
    svg.push_str(&svg_template::svg_header(width, height));
    svg.push_str(&svg_template::svg_prelude(
        "#eeeeee",
        "#eeeeb0",
        "Verdana",
        12,
        "rgb(0,0,0)",
        "rgb(160,160,160)",
        17,
    ));
    svg.push_str(&svg_template::svg_script(
        "Span:",
        12,
        0.59,
        10,
        0,
        "rgb(230,0,230)",
    ));
    svg.push_str(&svg_template::svg_controls(
        caption,
        "",
        width,
        height,
        17,
        12,
        "Verdana",
        "rgb(0,0,0)",
        10,
        30,
    ));
    svg.push_str(r#"<g id="frames">"#);
    nodes.for_each(|n| {
        let Node {
            title,
            samples,
            dur,
            percent,
            x,
            y,
            width: node_width,
            height,
            rgb,
        } = n;
        svg.push_str(&svg_template::svg_node(
            title, samples, dur, percent, x, y, node_width, height, rgb,
        ));
    });
    svg.push_str("</g>\n");
    svg.push_str(svg_template::SVG_FOOTER);
    Svg { svg }
}
