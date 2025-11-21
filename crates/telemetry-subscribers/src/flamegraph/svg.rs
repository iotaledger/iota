// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::time::Duration;

#[cfg(feature = "flamegraph-alloc")]
use super::alloc::AllocMetrics;

#[path = "svg_template.rs"]
mod svg_template;

#[derive(Clone, Debug)]
struct Node {
    title: String,
    x: f64,
    y: usize,
    width: f64,
    height: usize,
    rgb: (u8, u8, u8),
}

#[derive(Clone, Copy, Debug)]
pub struct Config {
    /// Desired resolution in units (nanoseconds, bytes) per pixel.
    pub resolution_units_per_px: Option<usize>,
    /// Desired width of the SVG in pixels.
    pub width: Option<usize>,
    /// Desired aspect ratio (width, height) of the SVG.
    pub aspect_ratio: Option<(usize, usize)>,
    /// Seed value for random color generation to ensure reproducible flamegraph
    /// colors.
    pub seed: u64,
    #[cfg(feature = "flamegraph-alloc")]
    /// Use memory allocations span measure instead of total duration.
    pub measure_mem: bool,
}
impl Default for Config {
    fn default() -> Config {
        Config {
            resolution_units_per_px: None,
            width: Some(1920),
            aspect_ratio: None,
            seed: 1,
            #[cfg(feature = "flamegraph-alloc")]
            measure_mem: false,
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

use super::{
    callgraph::{CallGraph, Frame, NodeId},
    flame::{Flames, FrameLabel, Metadata},
    metric::FlameMetric,
};

trait FromSpanMetrics<S>:
    for<'a> std::ops::Add<&'a S, Output = Self> + for<'a> std::ops::AddAssign<&'a S>
{
}

// Helper trait to abstract over span measure (eg. duration or allocations).
trait Measure: Clone + Copy + Default + Eq + for<'a> From<&'a RawNode> + Into<f64> + Sized {}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct TotalTime(Duration);
impl std::ops::Add<&FlameMetric> for TotalTime {
    type Output = Self;
    fn add(mut self, rhs: &FlameMetric) -> Self {
        self += rhs;
        self
    }
}
impl std::ops::AddAssign<&FlameMetric> for TotalTime {
    fn add_assign(&mut self, rhs: &FlameMetric) {
        self.0 += rhs.running.total;
    }
}
impl FromSpanMetrics<FlameMetric> for TotalTime {}
impl From<&RawNode> for TotalTime {
    fn from(raw: &RawNode) -> Self {
        TotalTime(raw.total)
    }
}
impl From<TotalTime> for f64 {
    fn from(t: TotalTime) -> f64 {
        t.0.as_nanos() as f64
    }
}
impl Measure for TotalTime {}

#[cfg(feature = "flamegraph-alloc")]
#[derive(Clone, Copy, Debug, Default)]
struct TotalMem(usize);
#[cfg(feature = "flamegraph-alloc")]
impl std::ops::Add<&FlameMetric> for TotalMem {
    type Output = Self;
    fn add(mut self, rhs: &FlameMetric) -> Self {
        self += rhs;
        self
    }
}
#[cfg(feature = "flamegraph-alloc")]
impl std::ops::AddAssign<&FlameMetric> for TotalMem {
    fn add_assign(&mut self, rhs: &FlameMetric) {
        self.0 += rhs.alloc_total.alloc;
    }
}
#[cfg(feature = "flamegraph-alloc")]
impl FromSpanMetrics<FlameMetric> for TotalMem {}
#[cfg(feature = "flamegraph-alloc")]
impl From<&RawNode> for TotalMem {
    fn from(raw: &RawNode) -> Self {
        TotalMem(raw.alloc.alloc)
    }
}
#[cfg(feature = "flamegraph-alloc")]
impl From<TotalMem> for f64 {
    fn from(t: TotalMem) -> f64 {
        t.0 as f64
    }
}
#[cfg(feature = "flamegraph-alloc")]
impl Measure for TotalMem {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RawNode {
    label: FrameLabel,
    samples: usize,
    total: Duration,
    #[cfg(feature = "flamegraph-alloc")]
    alloc: AllocMetrics,
}
impl From<&Frame<FlameMetric>> for RawNode {
    fn from(frame: &Frame<FlameMetric>) -> Self {
        RawNode {
            label: frame.label,
            samples: frame.metrics.count.entered,
            total: frame.metrics.running.total,
            #[cfg(feature = "flamegraph-alloc")]
            alloc: frame.metrics.alloc_total,
        }
    }
}
impl RawNode {
    fn into_node<M: Measure, R: rand::Rng>(
        self,
        start: M,
        overall: M,
        x_scale: f64,
        y: usize,
        rng: &mut R,
    ) -> Node {
        // start represents an absolute unscaled x coordinate of the span node in svg
        let x = start.into() * x_scale + 10.0;
        // width is the span node measure, ie. width in svg
        let width = M::from(&self);
        // overall is the measure of the whole flamegraph, represents 100%
        let percent = width.into() * 100.0 / overall.into();
        // scale width into pixels
        let width = width.into() * x_scale;
        // svg node height is fixed
        let height = 15;

        let RawNode {
            label,
            samples,
            total,
            #[cfg(feature = "flamegraph-alloc")]
            alloc,
        } = self;

        let rgb = random_rgb(rng);
        let dur = total.as_nanos() as f64 / 1_000_000.0;
        let avg = dur / samples as f64;
        #[cfg(not(feature = "flamegraph-alloc"))]
        let title = format!(
            "{} (#{samples}, dur={dur:.2}ms, avg={avg:.2}ms, {percent:.2}%)",
            label.name
        );
        #[cfg(feature = "flamegraph-alloc")]
        let title = format!(
            "{} (#{samples}, dur={dur:.2}ms, avg={avg:.2}ms, {percent:.2}%, {alloc})",
            label.name,
        );

        Node {
            title,
            x,
            y,
            width,
            height,
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
struct Raw<M> {
    total: M,
    running: Vec<Vec<(M, RawNode)>>,
}
impl<M: Measure> Raw<M> {
    fn add_node<S>(&mut self, frame: &Frame<S>, start: M, level: usize) -> M
    where
        for<'a> &'a Frame<S>: Into<RawNode>,
    {
        if self.running.len() <= level {
            self.running.resize(level + 1, Vec::new());
        }
        self.running[level].push((start, frame.into()));
        start
    }
    fn render(self, caption: &str, config: &Config) -> Svg {
        let Raw { total, running } = self;
        let num_levels = running.len();
        // 33px margins and 16px row height are hardcoded
        let height = 33 + num_levels * 16 + 33;

        let (x_scale, width) = config
            .resolution_units_per_px // try resolution first
            .and_then(|r| (r > 0).then_some(r))
            .map(|r| (1.0 / r as f64, total.into() as usize / r + 20))
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
                if total == M::default() {
                    (1.0, w)
                } else {
                    ((w - 20) as f64 / total.into(), w)
                }
            });

        use rand::SeedableRng as _;
        let mut rng = rand::rngs::SmallRng::seed_from_u64(config.seed);

        let nodes = running
            .into_iter()
            .enumerate()
            .flat_map(|(i, row)| row.into_iter().map(move |raw| (raw, i)))
            .map(|((start, raw), level)| {
                let y = (num_levels - 1 - level) * 16 + 33;
                raw.into_node(start, total, x_scale, y, &mut rng)
            });

        render(caption, width, height, nodes)
    }
}

impl CallGraph<FlameMetric> {
    fn raw_svg<M>(&self, raw: &mut Raw<M>)
    where
        M: Measure + FromSpanMetrics<FlameMetric>,
    {
        if !self.graph.is_empty() {
            let start = raw.total;
            let root_metrics = &self.graph[NodeId::default()].value.metrics;
            raw.total += root_metrics;

            self.graph.dfs_fold2(
                raw,
                || start,
                |svg_raw, start, node_id, level| {
                    svg_raw.add_node(&self.graph[node_id].value, start, level)
                },
                |_, start, node_id| {
                    let metrics = &self.graph[node_id].value.metrics;
                    *start + metrics
                },
            );
        }
    }
    fn render_svg_with_measure<M>(&self, caption: &str, config: &Config) -> Svg
    where
        M: Measure + FromSpanMetrics<FlameMetric>,
    {
        let mut raw = Raw::<M>::default();
        self.raw_svg(&mut raw);
        raw.render(caption, config)
    }
    fn render_svg(&self, caption: &str, config: &Config) -> Svg where {
        #[cfg(feature = "flamegraph-alloc")]
        {
            if config.measure_mem {
                self.render_svg_with_measure::<TotalMem>(caption, config)
            } else {
                self.render_svg_with_measure::<TotalTime>(caption, config)
            }
        }
        #[cfg(not(feature = "flamegraph-alloc"))]
        {
            self.render_svg_with_measure::<TotalTime>(caption, config)
        }
    }
}

impl Flames<FlameMetric> {
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
    fn render_combined_svg_with_measure<M>(
        &self,
        caption: &str,
        running: bool,
        completed: bool,
        config: &Config,
    ) -> Option<Svg>
    where
        M: Measure + FromSpanMetrics<FlameMetric>,
    {
        let mut raw = self.get_callgraphs(running, completed).values().fold(
            Raw::<M>::default(),
            |mut raw, callgraph| {
                callgraph.raw_svg(&mut raw);
                raw
            },
        );
        if raw.running.is_empty() {
            // no data at all
            None
        } else {
            // aggregate level 0 nodes into one
            let mut root = RawNode {
                label: FrameLabel { name: "all" },
                samples: 1,
                total: Duration::default(),
                #[cfg(feature = "flamegraph-alloc")]
                alloc: AllocMetrics::new(),
            };
            let level0 = raw.running.first().unwrap();
            for (_, node) in level0 {
                root.total += node.total;
                #[cfg(feature = "flamegraph-alloc")]
                root.alloc.merge(node.alloc);
            }
            // insert the root node at level 0 and shift other nodes 1 level up
            raw.running.insert(0, vec![(Default::default(), root)]);
            Some(raw.render(caption, config))
        }
    }
    pub fn render_combined_svg(
        &self,
        caption: &str,
        running: bool,
        completed: bool,
        config: &Config,
    ) -> Option<Svg> {
        #[cfg(feature = "flamegraph-alloc")]
        {
            if config.measure_mem {
                self.render_combined_svg_with_measure::<TotalMem>(
                    caption, running, completed, config,
                )
            } else {
                self.render_combined_svg_with_measure::<TotalTime>(
                    caption, running, completed, config,
                )
            }
        }
        #[cfg(not(feature = "flamegraph-alloc"))]
        {
            self.render_combined_svg_with_measure::<TotalTime>(caption, running, completed, config)
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
            x,
            y,
            width: node_width,
            height,
            rgb,
        } = n;
        svg.push_str(&svg_template::svg_node(
            &title, x, y, node_width, height, rgb,
        ));
    });
    svg.push_str("</g>\n");
    svg.push_str(svg_template::SVG_FOOTER);
    Svg { svg }
}
