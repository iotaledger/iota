// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::time::Duration;

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
            width: Some(3600),
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
    svg.push_str(XML_HEADER);
    svg.push_str(&svg_header(width, height));
    svg.push_str(SVG_PRELUDE);
    svg.push_str(SVG_SCRIPT);
    svg.push_str(&svg_controls(caption, width, height));
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
        svg.push_str(&svg_node(
            title, samples, dur, percent, x, y, node_width, height, rgb,
        ));
    });
    svg.push_str(SVG_FOOTER);
    Svg { svg }
}

const XML_HEADER: &str = r###"<?xml version="1.0" standalone="no"?>
<!DOCTYPE svg PUBLIC "-//W3C//DTD SVG 1.1//EN" "http://www.w3.org/Graphics/SVG/1.1/DTD/svg11.dtd">"###;

fn svg_header(width: usize, height: usize) -> String {
    format!(
        r###"<svg version="1.1" width="{width}" height="{height}" onload="init(evt)" viewBox="0 0 {width} {height}" xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink">"###
    )
}

const SVG_PRELUDE: &str = r###"
<defs >
	<linearGradient id="background" y1="0" y2="1" x1="0" x2="0" >
		<stop stop-color="#eeeeee" offset="5%" />
		<stop stop-color="#eeeeb0" offset="95%" />
	</linearGradient>
</defs>
<style type="text/css">
	.func_g:hover { stroke:black; stroke-width:0.5; }
</style>
"###;

const SVG_SCRIPT: &str = r###"
<script type="text/ecmascript">
<![CDATA[
	var details, searchbtn, matchedtxt, svg;
	function init(evt) { 
		details = document.getElementById("details").firstChild; 
		searchbtn = document.getElementById("search");
		matchedtxt = document.getElementById("matched");
		svg = document.getElementsByTagName("svg")[0];
		searching = 0;
	}

	// mouse-over for info
	function s(node) {		// show
		info = g_to_text(node);
		details.nodeValue = "Span: " + info;
	}
	function c() {			// clear
		details.nodeValue = ' ';
	}

	// ctrl-F for search
	window.addEventListener("keydown",function (e) {
		if (e.keyCode === 114 || (e.ctrlKey && e.keyCode === 70)) {
			e.preventDefault();
			search_prompt();
		}
	})

	// functions
	function find_child(parent, name, attr) {
		var children = parent.childNodes;
		for (var i=0; i<children.length;i++) {
			if (children[i].tagName == name)
				return (attr != undefined) ? children[i].attributes[attr].value : children[i];
		}
		return;
	}
	function orig_save(e, attr, val) {
		if (e.attributes["_orig_"+attr] != undefined) return;
		if (e.attributes[attr] == undefined) return;
		if (val == undefined) val = e.attributes[attr].value;
		e.setAttribute("_orig_"+attr, val);
	}
	function orig_load(e, attr) {
		if (e.attributes["_orig_"+attr] == undefined) return;
		e.attributes[attr].value = e.attributes["_orig_"+attr].value;
		e.removeAttribute("_orig_"+attr);
	}
	function g_to_text(e) {
		var text = find_child(e, "title").firstChild.nodeValue;
		return (text)
	}
	function g_to_func(e) {
		var func = g_to_text(e);
		if (func != null)
			func = func.replace(/ .*/, "");
		return (func);
	}
	function update_text(e) {
		var r = find_child(e, "rect");
		var t = find_child(e, "text");
		var w = parseFloat(r.attributes["width"].value) -3;
		var txt = find_child(e, "title").textContent.replace(/\([^(]*\)/,"");
		t.attributes["x"].value = parseFloat(r.attributes["x"].value) +3;
		
		// Smaller than this size won't fit anything
		if (w < 2*12*0.59) {
			t.textContent = "";
			return;
		}
		
		t.textContent = txt;
		// Fit in full text width
		if (/^ *$/.test(txt) || t.getSubStringLength(0, txt.length) < w)
			return;
		
		for (var x=txt.length-2; x>0; x--) {
			if (t.getSubStringLength(0, x+2) <= w) { 
				t.textContent = txt.substring(0,x) + "..";
				return;
			}
		}
		t.textContent = "";
	}

	// zoom
	function zoom_reset(e) {
		if (e.attributes != undefined) {
			orig_load(e, "x");
			orig_load(e, "width");
		}
		if (e.childNodes == undefined) return;
		for(var i=0, c=e.childNodes; i<c.length; i++) {
			zoom_reset(c[i]);
		}
	}
	function zoom_child(e, x, ratio) {
		if (e.attributes != undefined) {
			if (e.attributes["x"] != undefined) {
				orig_save(e, "x");
				e.attributes["x"].value = (parseFloat(e.attributes["x"].value) - x - 10) * ratio + 10;
				if(e.tagName == "text") e.attributes["x"].value = find_child(e.parentNode, "rect", "x") + 3;
			}
			if (e.attributes["width"] != undefined) {
				orig_save(e, "width");
				e.attributes["width"].value = parseFloat(e.attributes["width"].value) * ratio;
			}
		}
		
		if (e.childNodes == undefined) return;
		for(var i=0, c=e.childNodes; i<c.length; i++) {
			zoom_child(c[i], x-10, ratio);
		}
	}
	function zoom_parent(e) {
		if (e.attributes) {
			if (e.attributes["x"] != undefined) {
				orig_save(e, "x");
				e.attributes["x"].value = 10;
			}
			if (e.attributes["width"] != undefined) {
				orig_save(e, "width");
				e.attributes["width"].value = parseInt(svg.width.baseVal.value) - (10*2);
			}
		}
		if (e.childNodes == undefined) return;
		for(var i=0, c=e.childNodes; i<c.length; i++) {
			zoom_parent(c[i]);
		}
	}
	function zoom(node) { 
		var attr = find_child(node, "rect").attributes;
		var width = parseFloat(attr["width"].value);
		var xmin = parseFloat(attr["x"].value);
		var xmax = parseFloat(xmin + width);
		var ymin = parseFloat(attr["y"].value);
		var ratio = (svg.width.baseVal.value - 2*10) / width;
		
		// XXX: Workaround for JavaScript float issues (fix me)
		var fudge = 0.0001;
		
		var unzoombtn = document.getElementById("unzoom");
		unzoombtn.style["opacity"] = "1.0";
		
		var el = document.getElementsByTagName("g");
		for(var i=0;i<el.length;i++){
			var e = el[i];
			var a = find_child(e, "rect").attributes;
			var ex = parseFloat(a["x"].value);
			var ew = parseFloat(a["width"].value);
			// Is it an ancestor
			if (0 == 0) {
				var upstack = parseFloat(a["y"].value) > ymin;
			} else {
				var upstack = parseFloat(a["y"].value) < ymin;
			}
			if (upstack) {
				// Direct ancestor
				if (ex <= xmin && (ex+ew+fudge) >= xmax) {
					e.style["opacity"] = "0.5";
					zoom_parent(e);
					e.onclick = function(e){unzoom(); zoom(this);};
					update_text(e);
				}
				// not in current path
				else
					e.style["display"] = "none";
			}
			// Children maybe
			else {
				// no common path
				if (ex < xmin || ex + fudge >= xmax) {
					e.style["display"] = "none";
				}
				else {
					zoom_child(e, xmin, ratio);
					e.onclick = function(e){zoom(this);};
					update_text(e);
				}
			}
		}
	}
	function unzoom() {
		var unzoombtn = document.getElementById("unzoom");
		unzoombtn.style["opacity"] = "0.0";
		
		var el = document.getElementsByTagName("g");
		for(i=0;i<el.length;i++) {
			el[i].style["display"] = "block";
			el[i].style["opacity"] = "1";
			zoom_reset(el[i]);
			update_text(el[i]);
		}
	}	

	// search
	function reset_search() {
		var el = document.getElementsByTagName("rect");
		for (var i=0; i < el.length; i++) {
			orig_load(el[i], "fill")
		}
	}
	function search_prompt() {
		if (!searching) {
			var term = prompt("Enter a search term (regexp " +
			    "allowed, eg: ^ext4_)", "");
			if (term != null) {
				search(term)
			}
		} else {
			reset_search();
			searching = 0;
			searchbtn.style["opacity"] = "0.1";
			searchbtn.firstChild.nodeValue = "Search"
			matchedtxt.style["opacity"] = "0.0";
			matchedtxt.firstChild.nodeValue = ""
		}
	}
	function search(term) {
		var re = new RegExp(term);
		var el = document.getElementsByTagName("g");
		var matches = new Object();
		var maxwidth = 0;
		for (var i = 0; i < el.length; i++) {
			var e = el[i];
			if (e.attributes["class"].value != "func_g")
				continue;
			var func = g_to_func(e);
			var rect = find_child(e, "rect");
			if (rect == null) {
				// the rect might be wrapped in an anchor
				// if nameattr href is being used
				if (rect = find_child(e, "a")) {
				    rect = find_child(rect, "rect");
				}
			}
			if (func == null || rect == null)
				continue;

			// Save max width. Only works as we have a root frame
			var w = parseFloat(rect.attributes["width"].value);
			if (w > maxwidth)
				maxwidth = w;

			if (func.match(re)) {
				// highlight
				var x = parseFloat(rect.attributes["x"].value);
				orig_save(rect, "fill");
				rect.attributes["fill"].value =
				    "rgb(230,0,230)";

				// remember matches
				if (matches[x] == undefined) {
					matches[x] = w;
				} else {
					if (w > matches[x]) {
						// overwrite with parent
						matches[x] = w;
					}
				}
				searching = 1;
			}
		}
		if (!searching)
			return;

		searchbtn.style["opacity"] = "1.0";
		searchbtn.firstChild.nodeValue = "Reset Search"

		// calculate percent matched, excluding vertical overlap
		var count = 0;
		var lastx = -1;
		var lastw = 0;
		var keys = Array();
		for (k in matches) {
			if (matches.hasOwnProperty(k))
				keys.push(k);
		}
		// sort the matched frames by their x location
		// ascending, then width descending
		keys.sort(function(a, b){
				return a - b;
			if (a < b || a > b)
				return a - b;
			return matches[b] - matches[a];
		});
		// Step through frames saving only the biggest bottom-up frames
		// thanks to the sort order. This relies on the tree property
		// where children are always smaller than their parents.
		for (var k in keys) {
			var x = parseFloat(keys[k]);
			var w = matches[keys[k]];
			if (x >= lastx + lastw) {
				count += w;
				lastx = x;
				lastw = w;
			}
		}
		// display matched percent
		matchedtxt.style["opacity"] = "1.0";
		pct = 100 * count / maxwidth;
		if (pct == 100)
			pct = "100"
		else
			pct = pct.toFixed(1)
		matchedtxt.firstChild.nodeValue = "Matched: " + pct + "%";
	}
	function searchover(e) {
		searchbtn.style["opacity"] = "1.0";
	}
	function searchout(e) {
		if (searching) {
			searchbtn.style["opacity"] = "1.0";
		} else {
			searchbtn.style["opacity"] = "0.1";
		}
	}
]]>
</script>
"###;

fn svg_controls(caption: &str, width: usize, height: usize) -> String {
    let middle = width / 2;
    let start = 10;
    let end = width - 10;
    let top = 24;
    let bottom = height - 17;
    format!(
        r###"
<rect x="0" y="0" width="{width}" height="{height}" fill="url(#background)"  />
<text text-anchor="middle" x="{middle}" y="{top}" font-size="17" font-family="Verdana" fill="rgb(0,0,0)" >{caption}</text>
<text text-anchor="start" x="{start}" y="{top}" font-size="12" font-family="Verdana" fill="rgb(0,0,0)" id="unzoom" onclick="unzoom()" style="opacity:0.0;cursor:pointer" >Reset Zoom</text>
<text text-anchor="end" x="{end}" y="{top}" font-size="12" font-family="Verdana" fill="rgb(0,0,0)" id="search" onmouseover="searchover()" onmouseout="searchout()" onclick="search_prompt()" style="opacity:0.1;cursor:pointer" >Search</text>
<text text-anchor="end" x="{end}" y="{bottom}" font-size="12" font-family="Verdana" fill="rgb(0,0,0)" id="matched" > </text>
<text text-anchor="start" x="{start}" y="{bottom}" font-size="12" font-family="Verdana" fill="rgb(0,0,0)" id="details" > </text>
"###
    )
}

fn svg_node(
    title: &str,
    samples: usize,
    dur: Duration,
    percent: f64,
    x: f64,
    y: usize,
    width: f64,
    height: usize,
    rgb: (u8, u8, u8),
) -> String {
    let (fill_r, fill_g, fill_b) = rgb;
    let text_x = x + 3.0;
    let text_y = y + 12;
    let dur = dur.as_nanos() as f64 / 1_000_000.0;
    let avg = if samples > 0 {
        dur / samples as f64
    } else {
        0.0
    };
    format!(
        r###"<g class="func_g" onmouseover="s(this)" onmouseout="c()" onclick="zoom(this)">
<title>{title} (#{samples}, tot={dur:.2}ms, avg={avg:.2}ms, {percent:.2}%)</title><rect x="{x:.2}" y="{y}" width="{width:.2}" height="{height}" fill="rgb({fill_r},{fill_g},{fill_b})" rx="2" ry="2" />
<text text-anchor="" x="{text_x:.2}" y="{text_y}" font-size="12" font-family="Verdana" fill="rgb(0,0,0)"></text>
</g>
"###
    )
}

const SVG_FOOTER: &str = r###"</svg>"###;
