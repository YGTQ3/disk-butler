#!/usr/bin/env python3
"""Generate a star-history SVG from a list of ISO-8601 starred_at timestamps.

Usage: gh api repos/OWNER/REPO/stargazers --paginate --jq '.[].starred_at' | python star-history.py > star-history.svg
"""
import sys
from datetime import datetime, timezone
from xml.sax.saxutils import escape

W, H = 820, 300
ML, MR, MT, MB = 62, 18, 34, 44  # margins
ACCENT = "#f97316"
GRID = "#e5e7eb"
TEXT = "#374151"


def parse(stream):
    stamps = []
    for raw in stream:
        line = raw.strip().lstrip("\ufeff")  # tolerate UTF-8 BOM
        if not line:
            continue
        t = line.replace("Z", "+00:00")
        stamps.append(datetime.fromisoformat(t).astimezone(timezone.utc))
    return stamps


def build(stamps):
    if not stamps:
        return None
    stamps.sort()
    # cumulative count per unique timestamp
    series = []  # (timestamp, count)
    seen = set()
    for i, s in enumerate(stamps, 1):
        if s not in seen:
            seen.add(s)
            series.append((s, i))
        else:
            # multiple stars at same instant: count them all at this point
            series[-1] = (s, i)
    return series


def svg(series):
    t0, t1 = series[0][0], series[-1][0]
    total = series[-1][1]
    span = (t1 - t0).total_seconds() or 1.0
    cw, ch = W - ML - MR, H - MT - MB

    def x(t):
        return ML + (t - t0).total_seconds() / span * cw

    def y(c):
        return MT + ch - (c / total) * ch if total else MT + ch

    # y-axis ticks: 4 nice round steps
    step = max(1, round(total / 4))
    yticks = list(range(0, total + 1, step)) or [0]
    if yticks[-1] != total:
        yticks.append(total)
    yticks = sorted(set(yticks))

    parts = [
        f'<svg xmlns="http://www.w3.org/2000/svg" width="{W}" height="{H}" viewBox="0 0 {W} {H}" font-family="-apple-system,Segoe UI,Roboto,sans-serif">',
        f'<text x="{ML}" y="22" font-size="15" font-weight="700" fill="{TEXT}">Star History</text>',
        f'<text x="{W - MR}" y="22" font-size="13" font-weight="600" fill="{ACCENT}" text-anchor="end">★ {total}</text>',
    ]
    # horizontal grid + labels
    for c in yticks:
        yy = y(c)
        parts.append(f'<line x1="{ML}" y1="{yy:.1f}" x2="{W - MR}" y2="{yy:.1f}" stroke="{GRID}" stroke-width="1"/>')
        parts.append(f'<text x="{ML - 8}" y="{yy + 4:.1f}" font-size="11" fill="{TEXT}" text-anchor="end">{c}</text>')
    # x-axis labels: first / middle / last date
    mid = t0 + (t1 - t0) / 2
    for t, anchor in ((t0, "start"), (mid, "middle"), (t1, "end")):
        xx = x(t)
        parts.append(f'<text x="{xx:.1f}" y="{H - MB + 18}" font-size="11" fill="{TEXT}" text-anchor="{anchor}">{t.strftime("%Y-%m-%d")}</text>')
    # area fill
    pts_area = [(x(s), y(c)) for s, c in series]
    d_area = "M" + " L".join(f"{px:.1f},{py:.1f}" for px, py in pts_area) + f" L{x(t1):.1f},{MT + ch:.1f} L{x(t0):.1f},{MT + ch:.1f} Z"
    parts.append(f'<path d="{d_area}" fill="{ACCENT}" opacity="0.12"/>')
    # line
    d_line = "M" + " L".join(f"{px:.1f},{py:.1f}" for px, py in pts_area)
    parts.append(f'<path d="{d_line}" fill="none" stroke="{ACCENT}" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"/>')
    # last point dot
    lx, ly = pts_area[-1]
    parts.append(f'<circle cx="{lx:.1f}" cy="{ly:.1f}" r="4" fill="{ACCENT}" stroke="#fff" stroke-width="1.5"/>')
    parts.append("</svg>")
    return "\n".join(parts)


def main():
    sys.stdout.reconfigure(encoding="utf-8")
    sys.stdin.reconfigure(encoding="utf-8")
    stamps = parse(sys.stdin)
    series = build(stamps)
    if series is None:
        print("<!-- no star data yet -->")
        return
    print(svg(series))


if __name__ == "__main__":
    main()
