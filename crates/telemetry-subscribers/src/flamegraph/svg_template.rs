// This file contains SVG/HTML/JavaScript template code originally from:
// https://github.com/brendangregg/FlameGraph/blob/41fee1f99f9276008b7cd112fca19dc3ea84ac32/flamegraph.pl
//
// HISTORY
//
// This was inspired by Neelakanth Nadgir's excellent function_call_graph.rb
// program, which visualized function entry and return trace events.  As Neel
// wrote: "The output displayed is inspired by Roch's CallStackAnalyzer which
// was in turn inspired by the work on vftrace by Jan Boerhout".  See:
// https://blogs.oracle.com/realneel/entry/visualizing_callstacks_via_dtrace_and
//
// Copyright 2016 Netflix, Inc.
// Copyright 2011 Joyent, Inc.  All rights reserved.
// Copyright 2011 Brendan Gregg.  All rights reserved.
//
// CDDL HEADER START
//
// The contents of this file are subject to the terms of the
// Common Development and Distribution License (the "License").
// You may not use this file except in compliance with the License.
//
// You can obtain a copy of the license at docs/cddl1.txt or
// http://opensource.org/licenses/CDDL-1.0.
// See the License for the specific language governing permissions
// and limitations under the License.
//
// When distributing Covered Code, include this CDDL HEADER in each
// file and include the License file at docs/cddl1.txt.
// If applicable, add the following below this CDDL HEADER, with the
// fields enclosed by brackets "[]" replaced with your own identifying
// information: Portions Copyright [yyyy] [name of copyright owner]
//
// CDDL HEADER END
//
// 11-Oct-2014	Adrien Mahieux	Added zoom.
// 21-Nov-2013   Shawn Sterling  Added consistent palette file option
// 17-Mar-2013   Tim Bunce       Added options and more tunables.
// 15-Dec-2011	Dave Pacheco	Support for frames with whitespace.
// 10-Sep-2011	Brendan Gregg	Created this.

fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

pub(super) const XML_HEADER: &str = r###"<?xml version="1.0" standalone="no"?>
<!DOCTYPE svg PUBLIC "-//W3C//DTD SVG 1.1//EN" "http://www.w3.org/Graphics/SVG/1.1/DTD/svg11.dtd">"###;

pub(super) fn svg_header(width: usize, height: usize) -> String {
    format!(
        r###"<svg version="1.1" width="{width}" height="{height}" onload="init(evt)" viewBox="0 0 {width} {height}" xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink">
<!-- Flame graph stack visualization. See https://github.com/brendangregg/FlameGraph for latest version, and http://www.brendangregg.com/flamegraphs.html for examples. -->"###
    )
}

pub(super) fn svg_prelude(
    bgcolor1: &str,
    bgcolor2: &str,
    fonttype: &str,
    fontsize: usize,
    black: &str,
    vdgrey: &str,
    titlesize: usize,
) -> String {
    format!(
        r###"<defs>
	<linearGradient id="background" y1="0" y2="1" x1="0" x2="0" >
		<stop stop-color="{bgcolor1}" offset="5%" />
		<stop stop-color="{bgcolor2}" offset="95%" />
	</linearGradient>
</defs>
<style type="text/css">
	text {{ font-family:{fonttype}; font-size:{fontsize}px; fill:{black}; }}
	.btn {{ cursor:pointer; }}
	.btn rect {{ fill:white; stroke:rgb(0,0,0); stroke-width:1; }}
	.btn:hover rect {{ fill:rgb(230,230,230); }}
	.btn.show rect {{ fill:rgb(200,200,200); }}
	#subtitle {{ text-anchor:middle; font-color:{vdgrey}; }}
	#title {{ text-anchor:middle; font-size:{titlesize}px}}
	#frames > *:hover {{ stroke:black; stroke-width:0.5; cursor:pointer; }}
	.hide {{ display:none; }}
	.parent {{ opacity:0.5; }}
	.checkbox rect {{ fill:white; stroke:rgb(0,0,0); stroke-width:1; }}
	.checkbox.checked rect {{ fill:rgb(0,120,215); }}
	.checkbox text {{ fill:white; font-weight:bold; pointer-events:none; }}
</style>
"###
    )
}

pub(super) fn svg_script(
    nametype: &str,
    fontsize: usize,
    fontwidth: f64,
    xpad: usize,
    inverted: usize,
    searchcolor: &str,
) -> String {
    {
        format!(
            r###"<script type="text/ecmascript">
<![CDATA[
	"use strict";
	var details, searchbtn, unzoombtn, matchedtxt, svg, searching, currentSearchTerm, ignorecase, ignorecaseBtn;
	function init(evt) {{
		details = document.getElementById("details").firstChild;
		searchbtn = document.getElementById("search");
		ignorecaseBtn = document.getElementById("ignorecase");
		unzoombtn = document.getElementById("unzoom");
		matchedtxt = document.getElementById("matched");
		svg = document.getElementsByTagName("svg")[0];
		searching = 0;
		currentSearchTerm = null;

		// use GET parameters to restore a flamegraphs state.
		var params = get_params();
		if (params.x && params.y)
			zoom(find_group(document.querySelector('[x="' + params.x + '"][y="' + params.y + '"]')));
                if (params.s) search(params.s);
	}}

	// event listeners
	window.addEventListener("click", function(e) {{
		// Check if click is on a button or its children
		var btn = e.target;
		while (btn && btn.id !== "search" && btn.id !== "ignorecase" && btn.id !== "unzoom") {{
			if (btn.parentElement && (btn.parentElement.id === "search" || btn.parentElement.id === "ignorecase" || btn.parentElement.id === "unzoom")) {{
				btn = btn.parentElement;
				break;
			}}
			if (btn === document.documentElement) break;
			btn = btn.parentElement;
		}}
		
		if (btn && btn.id == "unzoom") {{
			clearzoom();
			return;
		}}
		if (btn && btn.id == "search") {{
			search_prompt();
			return;
		}}
		if (btn && btn.id == "ignorecase") {{
			toggle_ignorecase();
			return;
		}}

		var target = find_group(e.target);
		if (target) {{
			if (target.nodeName == "a") {{
				if (e.ctrlKey === false) return;
				e.preventDefault();
			}}
			if (target.classList.contains("parent")) unzoom(true);
			zoom(target);
			if (!document.querySelector('.parent')) {{
				// we have basically done a clearzoom so clear the url
				var params = get_params();
				if (params.x) delete params.x;
				if (params.y) delete params.y;
				history.replaceState(null, null, parse_params(params));
				unzoombtn.classList.add("hide");
				return;
			}}

			// set parameters for zoom state
			var el = target.querySelector("rect");
			if (el && el.attributes && el.attributes.y && el.attributes._orig_x) {{
				var params = get_params()
				params.x = el.attributes._orig_x.value;
				params.y = el.attributes.y.value;
				history.replaceState(null, null, parse_params(params));
			}}
		}}
	}}, false)

	// mouse-over for info
	// show
	window.addEventListener("mouseover", function(e) {{
		var target = find_group(e.target);
		if (target) details.nodeValue = "{nametype} " + g_to_text(target);
	}}, false)

	// clear
	window.addEventListener("mouseout", function(e) {{
		var target = find_group(e.target);
		if (target) details.nodeValue = ' ';
	}}, false)

	// ctrl-F for search
	// ctrl-I to toggle case-sensitive search
	window.addEventListener("keydown",function (e) {{
		if (e.keyCode === 114 || (e.ctrlKey && e.keyCode === 70)) {{
			e.preventDefault();
			search_prompt();
		}}
		else if (e.ctrlKey && e.keyCode === 73) {{
			e.preventDefault();
			toggle_ignorecase();
		}}
	}}, false)

	// functions
	function get_params() {{
		var params = {{}};
		var paramsarr = window.location.search.substr(1).split('&');
		for (var i = 0; i < paramsarr.length; ++i) {{
			var tmp = paramsarr[i].split("=");
			if (!tmp[0] || !tmp[1]) continue;
			params[tmp[0]]  = decodeURIComponent(tmp[1]);
		}}
		return params;
	}}
	function parse_params(params) {{
		var uri = "?";
		for (var key in params) {{
			uri += key + '=' + encodeURIComponent(params[key]) + '&';
		}}
		if (uri.slice(-1) == "&")
			uri = uri.substring(0, uri.length - 1);
		if (uri == '?')
			uri = window.location.href.split('?')[0];
		return uri;
	}}
	function find_child(node, selector) {{
		var children = node.querySelectorAll(selector);
		if (children.length) return children[0];
	}}
	function find_group(node) {{
		var parent = node.parentElement;
		if (!parent) return;
		if (parent.id == "frames") return node;
		return find_group(parent);
	}}
	function orig_save(e, attr, val) {{
		if (e.attributes["_orig_" + attr] != undefined) return;
		if (e.attributes[attr] == undefined) return;
		if (val == undefined) val = e.attributes[attr].value;
		e.setAttribute("_orig_" + attr, val);
	}}
	function orig_load(e, attr) {{
		if (e.attributes["_orig_"+attr] == undefined) return;
		e.attributes[attr].value = e.attributes["_orig_" + attr].value;
		e.removeAttribute("_orig_"+attr);
	}}
	function g_to_text(e) {{
		var text = find_child(e, "title").firstChild.nodeValue;
		return (text)
	}}
	function g_to_func(e) {{
		var func = g_to_text(e);
		// if there's any manipulation we want to do to the function
		// name before it's searched, do it here before returning.
		return (func);
	}}
	function update_text(e) {{
		var r = find_child(e, "rect");
		var t = find_child(e, "text");
		var w = parseFloat(r.attributes.width.value) -3;
		var txt = find_child(e, "title").textContent.replace(/\\([^(]*\\)\$/,"");
		t.attributes.x.value = parseFloat(r.attributes.x.value) + 3;

		// Smaller than this size won't fit anything
		if (w < 2 * {fontsize} * {fontwidth}) {{
			t.textContent = "";
			return;
		}}

		t.textContent = txt;
		var sl = t.getSubStringLength(0, txt.length);
		// check if only whitespace or if we can fit the entire string into width w
		if (/^ *\$/.test(txt) || sl < w)
			return;

		// this isn't perfect, but gives a good starting point
		// and avoids calling getSubStringLength too often
		var start = Math.floor((w/sl) * txt.length);
		for (var x = start; x > 0; x = x-2) {{
			if (t.getSubStringLength(0, x + 2) <= w) {{
				t.textContent = txt.substring(0, x) + "..";
				return;
			}}
		}}
		t.textContent = "";
	}}

	// zoom
	function zoom_reset(e) {{
		if (e.attributes != undefined) {{
			orig_load(e, "x");
			orig_load(e, "width");
		}}
		if (e.childNodes == undefined) return;
		for (var i = 0, c = e.childNodes; i < c.length; i++) {{
			zoom_reset(c[i]);
		}}
	}}
	function zoom_child(e, x, ratio) {{
		if (e.attributes != undefined) {{
			if (e.attributes.x != undefined) {{
				orig_save(e, "x");
				e.attributes.x.value = (parseFloat(e.attributes.x.value) - x - {xpad}) * ratio + {xpad};
				if (e.tagName == "text")
					e.attributes.x.value = find_child(e.parentNode, "rect[x]").attributes.x.value + 3;
			}}
			if (e.attributes.width != undefined) {{
				orig_save(e, "width");
				e.attributes.width.value = parseFloat(e.attributes.width.value) * ratio;
			}}
		}}

		if (e.childNodes == undefined) return;
		for (var i = 0, c = e.childNodes; i < c.length; i++) {{
			zoom_child(c[i], x - {xpad}, ratio);
		}}
	}}
	function zoom_parent(e) {{
		if (e.attributes) {{
			if (e.attributes.x != undefined) {{
				orig_save(e, "x");
				e.attributes.x.value = {xpad};
			}}
			if (e.attributes.width != undefined) {{
				orig_save(e, "width");
				e.attributes.width.value = parseInt(svg.width.baseVal.value) - ({xpad} * 2);
			}}
		}}
		if (e.childNodes == undefined) return;
		for (var i = 0, c = e.childNodes; i < c.length; i++) {{
			zoom_parent(c[i]);
		}}
	}}
	function zoom(node) {{
		var attr = find_child(node, "rect").attributes;
		var width = parseFloat(attr.width.value);
		var xmin = parseFloat(attr.x.value);
		var xmax = parseFloat(xmin + width);
		var ymin = parseFloat(attr.y.value);
		var ratio = (svg.width.baseVal.value - 2 * {xpad}) / width;

		// XXX: Workaround for JavaScript float issues (fix me)
		var fudge = 0.0001;

		unzoombtn.classList.remove("hide");

		var el = document.getElementById("frames").children;
		for (var i = 0; i < el.length; i++) {{
			var e = el[i];
			var a = find_child(e, "rect").attributes;
			var ex = parseFloat(a.x.value);
			var ew = parseFloat(a.width.value);
			var upstack;
			// Is it an ancestor
			if ({inverted} == 0) {{
				upstack = parseFloat(a.y.value) > ymin;
			}} else {{
				upstack = parseFloat(a.y.value) < ymin;
			}}
			if (upstack) {{
				// Direct ancestor
				if (ex <= xmin && (ex+ew+fudge) >= xmax) {{
					e.classList.add("parent");
					zoom_parent(e);
					update_text(e);
				}}
				// not in current path
				else
					e.classList.add("hide");
			}}
			// Children maybe
			else {{
				// no common path
				if (ex < xmin || ex + fudge >= xmax) {{
					e.classList.add("hide");
				}}
				else {{
					zoom_child(e, xmin, ratio);
					update_text(e);
				}}
			}}
		}}
		search();
	}}
	function unzoom(dont_update_text) {{
		unzoombtn.classList.add("hide");
		var el = document.getElementById("frames").children;
		for(var i = 0; i < el.length; i++) {{
			el[i].classList.remove("parent");
			el[i].classList.remove("hide");
			zoom_reset(el[i]);
			if(!dont_update_text) update_text(el[i]);
		}}
		search();
	}}
	function clearzoom() {{
		unzoom();

		// remove zoom state
		var params = get_params();
		if (params.x) delete params.x;
		if (params.y) delete params.y;
		history.replaceState(null, null, parse_params(params));
	}}

	// search
	function toggle_ignorecase() {{
		ignorecase = !ignorecase;
		if (ignorecase) {{
			ignorecaseBtn.classList.add("checked");
		}} else {{
			ignorecaseBtn.classList.remove("checked");
		}}
		reset_search();
		search();
	}}
	function reset_search() {{
		var el = document.querySelectorAll("#frames rect");
		for (var i = 0; i < el.length; i++) {{
			orig_load(el[i], "fill")
		}}
		var params = get_params();
		delete params.s;
		history.replaceState(null, null, parse_params(params));
	}}
	function search_prompt() {{
		if (!searching) {{
			var term = prompt("Enter a search term (regexp " +
			    "allowed, eg: ^ext4_)"
			    + (ignorecase ? ", ignoring case" : "")
			    + "\nPress Ctrl-i to toggle case sensitivity", "");
			if (term != null) search(term);
		}} else {{
			reset_search();
			searching = 0;
			currentSearchTerm = null;
			searchbtn.classList.remove("show");
			searchbtn.firstChild.nodeValue = "Search"
			matchedtxt.classList.add("hide");
			matchedtxt.firstChild.nodeValue = ""
		}}
	}}
	function search(term) {{
		if (term) currentSearchTerm = term;
		if (currentSearchTerm === null) return;

		var re = new RegExp(currentSearchTerm, ignorecase ? 'i' : '');
		var el = document.getElementById("frames").children;
		var matches = new Object();
		var maxwidth = 0;
		for (var i = 0; i < el.length; i++) {{
			var e = el[i];
			var func = g_to_func(e);
			var rect = find_child(e, "rect");
			if (func == null || rect == null)
				continue;

			// Save max width. Only works as we have a root frame
			var w = parseFloat(rect.attributes.width.value);
			if (w > maxwidth)
				maxwidth = w;

			if (func.match(re)) {{
				// highlight
				var x = parseFloat(rect.attributes.x.value);
				orig_save(rect, "fill");
				rect.attributes.fill.value = "{searchcolor}";

				// remember matches
				if (matches[x] == undefined) {{
					matches[x] = w;
				}} else {{
					if (w > matches[x]) {{
						// overwrite with parent
						matches[x] = w;
					}}
				}}
				searching = 1;
			}}
		}}
		if (!searching)
			return;
		var params = get_params();
		params.s = currentSearchTerm;
		history.replaceState(null, null, parse_params(params));

		searchbtn.classList.add("show");
		searchbtn.firstChild.nodeValue = "Reset Search";

		// calculate percent matched, excluding vertical overlap
		var count = 0;
		var lastx = -1;
		var lastw = 0;
		var keys = Array();
		for (k in matches) {{
			if (matches.hasOwnProperty(k))
				keys.push(k);
		}}
		// sort the matched frames by their x location
		// ascending, then width descending
		keys.sort(function(a, b){{
			return a - b;
		}});
		// Step through frames saving only the biggest bottom-up frames
		// thanks to the sort order. This relies on the tree property
		// where children are always smaller than their parents.
		var fudge = 0.0001;	// JavaScript floating point
		for (var k in keys) {{
			var x = parseFloat(keys[k]);
			var w = matches[keys[k]];
			if (x >= lastx + lastw - fudge) {{
				count += w;
				lastx = x;
				lastw = w;
			}}
		}}
		// display matched percent
		matchedtxt.classList.remove("hide");
		var pct = 100 * count / maxwidth;
		if (pct != 100) pct = pct.toFixed(1)
		matchedtxt.firstChild.nodeValue = "Matched: " + pct + "%";
	}}
]]>
</script>
"###,
            nametype = nametype,
            fontsize = fontsize,
            fontwidth = fontwidth,
            xpad = xpad,
            inverted = inverted,
            searchcolor = searchcolor
        )
    }
}

pub(super) fn svg_controls(
    title: &str,
    subtitle: &str,
    width: usize,
    height: usize,
    titlesize: usize,
    fontsize: usize,
    fonttype: &str,
    textcolor: &str,
    xpad: usize,
    ypad2: usize,
) -> String {
    let middle = width / 2;
    let top_title = fontsize * 2;
    let top_subtitle = fontsize * 4;
    let bottom = height - (ypad2 / 2);

    let escaped_title = escape_xml(title);
    let subtitle_elem = if !subtitle.is_empty() {
        let escaped_subtitle = escape_xml(subtitle);
        format!(
            r#"<text id="subtitle" x="{middle}" y="{top_subtitle}" font-size="{fontsize}" font-family="{fonttype}" fill="{textcolor}" >{escaped_subtitle}</text>"#
        )
    } else {
        String::new()
    };

    // Search button aligned to right edge
    let search_btn_width = 60;
    let search_btn_x = width - xpad - search_btn_width - 5;
    let search_text_x = width - xpad - search_btn_width;
    let btn_y = top_title - fontsize;
    let btn_height = fontsize + 4;

    // Reset Zoom button aligned to left edge
    let unzoom_btn_width = 80;
    let unzoom_btn_x = xpad - 5;

    // Checkbox and label centered below Search button
    let checkbox_size = 14;
    let label_text = "ignore case";
    let label_width = label_text.len() as f64 * fontsize as f64 * 0.6; // Approximate text width
    let checkbox_group_width = checkbox_size as f64 + 5.0 + label_width;
    let search_btn_center = search_btn_x + (search_btn_width / 2);
    let checkbox_group_start = search_btn_center - (checkbox_group_width as usize / 2);

    let checkbox_x = checkbox_group_start;
    let checkbox_y = top_title + 5;
    let checkmark_x = checkbox_x + 3;
    let checkmark_y = checkbox_y + 11;
    let checkbox_label_x = checkbox_x + checkbox_size + 5;
    let checkbox_label_y = checkbox_y + 11; // Align with checkmark baseline

    format!(
        r###"
<rect x="0" y="0" width="{width}" height="{height}" fill="url(#background)"  />
<text id="title" x="{middle}" y="{top_title}" font-size="{titlesize}" font-family="{fonttype}" fill="{textcolor}" >{escaped_title}</text>
{subtitle_elem}
<text id="details" x="{xpad}" y="{bottom}" font-size="{fontsize}" font-family="{fonttype}" fill="{textcolor}" > </text>
<g id="unzoom" class="btn hide">
	<rect x="{unzoom_btn_x}" y="{btn_y}" width="{unzoom_btn_width}" height="{btn_height}" rx="2" ry="2" />
	<text x="{xpad}" y="{top_title}" font-size="{fontsize}" font-family="{fonttype}" fill="{textcolor}" >Reset Zoom</text>
</g>
<g id="search" class="btn">
	<rect x="{search_btn_x}" y="{btn_y}" width="{search_btn_width}" height="{btn_height}" rx="2" ry="2" />
	<text x="{search_text_x}" y="{top_title}" font-size="{fontsize}" font-family="{fonttype}" fill="{textcolor}" >Search</text>
</g>
<g id="ignorecase" class="btn checkbox" style="cursor:pointer;">
	<rect x="{checkbox_x}" y="{checkbox_y}" width="{checkbox_size}" height="{checkbox_size}" rx="2" ry="2" />
	<text x="{checkmark_x}" y="{checkmark_y}" font-size="11" font-family="{fonttype}" >✓</text>
</g>
<text x="{checkbox_label_x}" y="{checkbox_label_y}" font-size="{fontsize}" font-family="{fonttype}" fill="{textcolor}" style="pointer-events:none;" >ignore case</text>
<text id="matched" x="{search_text_x}" y="{bottom}" font-size="{fontsize}" font-family="{fonttype}" fill="{textcolor}" class="hide"> </text>
"###
    )
}

pub(super) fn svg_node(
    title: &str,
    samples: usize,
    dur: std::time::Duration,
    percent: f64,
    x: f64,
    y: usize,
    width: f64,
    height: usize,
    rgb: (u8, u8, u8),
) -> String {
    let (fill_r, fill_g, fill_b) = rgb;
    let text_x = x + 3.0;
    let text_y = (y as f64) + (height as f64 / 2.0) + 3.0;
    let dur = dur.as_nanos() as f64 / 1_000_000.0;
    let avg = if samples > 0 {
        dur / samples as f64
    } else {
        0.0
    };

    let escaped_title = escape_xml(title);

    format!(
        r###"<g class="func_g">
<title>{escaped_title} (#{samples}, tot={dur:.2}ms, avg={avg:.2}ms, {percent:.2}%)</title>
<rect x="{x:.1}" y="{y}" width="{width:.1}" height="{height}" fill="rgb({fill_r},{fill_g},{fill_b})" rx="2" ry="2" />
<text x="{text_x:.2}" y="{text_y:.0}"></text>
</g>
"###
    )
}

pub(super) const SVG_FOOTER: &str = r###"</svg>"###;
