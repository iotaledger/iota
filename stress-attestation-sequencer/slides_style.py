"""slides_style.py — the talk deck's look, for the slide figures.

Colours, fonts and text sizes are the ones the deck (Transaction Attestation,
13 October 2026) uses. A figure is drawn at the size of the space under a slide
title, 16.5 × 6.2 in on the 20 × 11.25 in slide, and saved with a 0.1 in
transparent margin on every side, since Google Slides cuts off what touches an
image's edge. Its text matches the deck's when the image is placed 1.66 in from
the left and 2.3 in from the top at 16.7 in wide; there it stays clear of the
logo in the bottom right corner. Each panel sits on a white card like the
deck's boxes, and the rest of the image is transparent, so the slide background
shows through.

Loaded by h1/slides-plot/plot.py and h2/slides-plot/plot.py.
"""

import io

import matplotlib.pyplot as plt
from matplotlib.patches import FancyBboxPatch
from matplotlib.ticker import FuncFormatter, LogLocator, NullLocator

# The deck's colours.
NAVY = "#131F37"  # titles and headings
SLATE = "#485776"  # body text
GREY = "#808EA7"  # captions
TEAL = "#0FC1B7"  # the accent
DARK_TEAL = "#0A9C94"  # teal text
LIGHT_TEAL = "#7FDAD3"
ORANGE = "#FA7B17"  # a warning
RED = "#EA4335"  # a failure
LINE = "#D5DDE7"  # box outlines
TINT = "#E9EEF4"  # grey box fill
PALE = "#F2F5F9"  # a lighter grey box fill
RED_TINT = "#FDECEA"  # a failure's box fill

# A white box behind a label, so grid and reference lines do not run through it.
LABEL_BOX = {"boxstyle": "square,pad=0.15", "fc": "white", "ec": "none"}

SIZE = (16.5, 6.2)  # inches
MARGIN = 0.1  # inches of empty border the saved image adds on every side
DPI = 200
GAP = 0.44  # between two cards, as on the deck's two-box slides
INSET = 0.34  # from a card's edge to its heading

plt.rcParams.update(
    {
        # Roboto Light is weight 300; matplotlib's "light" is 200, which picks
        # Roboto Thin.
        "font.family": "Roboto",
        "font.weight": 300,
        "font.size": 15,
        "text.color": SLATE,
        "axes.labelcolor": SLATE,
        "axes.labelsize": 15,
        "axes.labelweight": 300,
        "axes.labelpad": 10,
        "axes.titlesize": 16,
        "axes.titleweight": "bold",
        "axes.titlecolor": NAVY,
        "axes.titlepad": 10,
        "axes.facecolor": "none",
        "axes.edgecolor": LINE,
        "axes.linewidth": 1.2,
        "axes.spines.top": False,
        "axes.spines.right": False,
        "axes.spines.left": False,
        "axes.axisbelow": True,
        "grid.color": TINT,
        "grid.linewidth": 1.2,
        "xtick.color": LINE,
        "ytick.color": LINE,
        "xtick.labelcolor": GREY,
        "ytick.labelcolor": GREY,
        "xtick.labelsize": 14,
        "ytick.labelsize": 14,
        "xtick.major.size": 0,
        "ytick.major.size": 0,
        "xtick.minor.size": 0,
        "ytick.minor.size": 0,
        "xtick.major.pad": 8,
        "ytick.major.pad": 8,
        "legend.fontsize": 15,
        "legend.handlelength": 1.6,
        # A white box with no border, so a legend over the plot hides the grid.
        "legend.frameon": True,
        "legend.framealpha": 1.0,
        "legend.facecolor": "white",
        "legend.edgecolor": "none",
        "legend.fancybox": False,
        "lines.linewidth": 3,
        "lines.markersize": 8,
        "figure.facecolor": "none",
        "savefig.facecolor": "none",
    }
)


def columns(ratios):
    """(x, width) in inches of cards side by side across the figure, their
    widths in these ratios."""
    free = SIZE[0] - GAP * (len(ratios) - 1)
    out, x = [], 0.0
    for r in ratios:
        w = free * r / sum(ratios)
        out.append((x, w))
        x += w + GAP
    return out


def card(fig, x, y, w, h, title=None, caption=None):
    """A white card at (x, y), w × h inches from the figure's bottom left,
    with a heading and a caption in its top left corner."""
    fig.add_artist(
        FancyBboxPatch(
            (x + 0.01, y + 0.01),
            w - 0.02,
            h - 0.02,
            boxstyle="round,pad=0,rounding_size=0.12",
            transform=fig.dpi_scale_trans,
            facecolor="white",
            edgecolor=LINE,
            lw=1.2,
            zorder=-1,
        )
    )
    top = y + h - 0.24
    if title:
        fig.text(
            x + INSET,
            top,
            title,
            transform=fig.dpi_scale_trans,
            va="top",
            fontsize=20,
            fontweight="bold",
            color=NAVY,
        )
        top -= 0.44
    if caption:
        fig.text(
            x + INSET,
            top,
            caption,
            transform=fig.dpi_scale_trans,
            va="top",
            color=SLATE,
        )


def subtitle(fig, text, note=None):
    """A line above the cards for what all of a figure's cards share, where
    the deck puts a subtitle under the slide title, and an optional second
    line in the caption colour. Returns the height left for the cards."""
    height = fig.get_size_inches()[1]
    fig.text(
        0,
        height - 0.02,
        text,
        transform=fig.dpi_scale_trans,
        va="top",
        fontsize=18,
        color=SLATE,
    )
    if note is None:
        return height - 0.55
    fig.text(
        0, height - 0.42, note, transform=fig.dpi_scale_trans, va="top", color=GREY
    )
    return height - 0.9


def subtitle_legend(fig, ax):
    """Move ax's legend to the right end of the subtitle line, for series all
    of a figure's cards share."""
    handles, labels = ax.get_legend_handles_labels()
    fig.legend(
        handles,
        labels,
        loc="upper right",
        bbox_to_anchor=(1, 1),
        ncol=len(labels),
        frameon=False,
        borderpad=0,
        borderaxespad=0,
    )


def panel(fig, x, y, w, h, title=None, caption=None, pad=(1.0, 0.8, 0.35, None)):
    """A card holding one axes, `pad` inches in from the card's left, bottom,
    right and top edges. The top pad defaults to the room the heading and
    caption take."""
    card(fig, x, y, w, h, title, caption)
    left, bottom, right, top = pad
    if top is None:
        top = 0.45 + (0.44 if title else 0) + (0.4 if caption else 0)
    fw, fh = fig.get_size_inches()
    return fig.add_axes(
        [
            (x + left) / fw,
            (y + bottom) / fh,
            (w - left - right) / fw,
            (h - bottom - top) / fh,
        ]
    )


def ink(color):
    """The colour to write a series' labels in: teal text is a shade darker
    than the line, as in the deck."""
    return {TEAL: DARK_TEAL, LIGHT_TEAL: TEAL}.get(color, color)


def label_column(ax, x, labels, dx, gap=20, **kw):
    """Write labels beside points that share one x, moved apart so no two are
    closer than `gap` points. `labels` is [(y, text, colour)]."""
    ax.get_ylim()  # settles the limits, so the transform below is final
    to_pt = 72 / ax.figure.dpi
    ys = [ax.transData.transform((x, y))[1] * to_pt for y, _, _ in labels]
    placed, last = {}, None
    for i in sorted(range(len(labels)), key=lambda i: ys[i]):
        placed[i] = ys[i] if last is None else max(ys[i], last + gap)
        last = placed[i]
    for i, (y, text, color) in enumerate(labels):
        ax.annotate(
            text,
            (x, y),
            xytext=(dx, placed[i] - ys[i]),
            textcoords="offset points",
            va="center",
            color=color,
            **kw,
        )


def units(v):
    """1000 -> 1K, 1500000 -> 1.5M."""
    if v >= 1e6:
        return f"{v / 1e6:g}M"
    if v >= 1e3:
        return f"{v / 1e3:g}K"
    return f"{v:g}"


def ms(v):
    """Milliseconds as an axis shows them: 0.1 ms, 10 ms, 1 s, 10 s."""
    return f"{v / 1000:g} s" if v >= 1000 else f"{v:g} ms"


def log_axis(axis, label):
    """A log axis with a tick at each power of ten, written by `label`."""
    axis.set_major_locator(LogLocator(base=10))
    axis.set_minor_locator(NullLocator())
    axis.set_major_formatter(FuncFormatter(lambda v, _: label(v)))


def save(fig, path):
    """Write the figure as a PNG with MARGIN of empty border on every side."""
    # Pillow comes with matplotlib.
    from PIL import Image, ImageOps

    buf = io.BytesIO()
    fig.savefig(buf, format="png", dpi=DPI, transparent=True)
    plt.close(fig)
    buf.seek(0)
    border = round(MARGIN * DPI)
    ImageOps.expand(Image.open(buf), border=border, fill=(0, 0, 0, 0)).save(
        path, dpi=(DPI, DPI)
    )
    print("wrote", path)
