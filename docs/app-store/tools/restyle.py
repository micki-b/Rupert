"""Writes a styled HTML page for each sample PDF: made-up letterhead, the
document's own text unchanged (so search finds the same words), a total box
when there's an amount. Chrome then prints them over the originals.

restyle.py <library> <html out dir> < pdftext output (name<TAB>text)
"""
import html
import re
import sys
from pathlib import Path

ISSUERS = {
    "Car purchase agreement": ("Lakeside Motors", "1400 Lakeshore Drive · Portland, OR", "#2f5d8a"),
    "Mortgage closing disclosure": ("Harbor Savings Bank", "Home Lending · 200 Harbor Way · Portland, OR", "#1f5f5b"),
    "Water bill": ("City Water", "Utility Billing · PO Box 2100 · Portland, OR", "#2a7ab0"),
    "Vaccination record": ("Northside Family Clinic", "45 Northside Avenue · Portland, OR", "#3f7d4e"),
    "Home insurance policy": ("Acme Mutual Insurance", "Homeowners · 900 Market Street · Portland, OR", "#8a3b2e"),
    "Property tax bill": ("Multnomah County Treasurer", "501 SE Hawthorne Blvd · Portland, OR", "#4a4a6a"),
    "Dental plan summary": ("BrightSmile Dental PPO", "Member Services · 1-800-555-0172", "#2b8a8a"),
    "Charitable donation receipts": ("City Food Bank", "17 Commerce Street · Portland, OR", "#b0652a"),
    "Vehicle registration": ("Oregon Driver & Motor Vehicle Services", "Salem, OR", "#2f4f7f"),
    "Umbrella policy": ("Acme Mutual Insurance", "Personal Lines · 900 Market Street · Portland, OR", "#8a3b2e"),
    "Running shoes": ("Trailhead Outfitters", "322 NW 23rd Avenue · Portland, OR", "#3d7a3a"),
    "Lab results": ("Northside Family Clinic Laboratory", "45 Northside Avenue · Portland, OR", "#3f7d4e"),
    "W-2 Northwind Traders": ("Northwind Traders", "Payroll · 1 Northwind Plaza · Seattle, WA", "#3b3b3b"),
    "1099-INT Harbor Savings": ("Harbor Savings Bank", "Tax Reporting · 200 Harbor Way · Portland, OR", "#1f5f5b"),
    "Pet insurance claim": ("Pawsure Pet Insurance", "Claims · 1-800-555-0133", "#7a4fa0"),
    "1099-DIV Brokerage": ("Summit Brokerage", "Tax Center · 77 Summit Street · Denver, CO", "#5a4632"),
    "Bike tune-up": ("Spoke and Chain Cycles", "88 SE Division Street · Portland, OR", "#c0392b"),
    "Car insurance renewal": ("Acme Mutual Insurance", "Auto · 900 Market Street · Portland, OR", "#8a3b2e"),
    "Car service": ("Lakeside Motors Service", "1400 Lakeshore Drive · Portland, OR", "#2f5d8a"),
    "Laptop": ("Brightline Electronics", "Lloyd Center · Portland, OR", "#34495e"),
    "Electric bill": ("City Electric", "Customer Service · PO Box 3300 · Portland, OR", "#d4a017"),
    "Eye exam prescription": ("Clearview Optometry", "610 NE Broadway · Portland, OR", "#2c6e9e"),
    "Dishwasher receipt": ("City Appliances", "4100 SE 82nd Avenue · Portland, OR", "#2d6a4f"),
    "Plumber invoice": ("Pipeline Plumbing", "Licensed & bonded · (503) 555-0161", "#1d4e89"),
    "Garden center": ("Greenleaf Garden Center", "7400 SW Barbur Blvd · Portland, OR", "#4c8c2b"),
    "Travel insurance Portugal": ("Wayfarer Travel Protection", "Policy Services · 1-800-555-0188", "#c06014"),
    "Roof warranty": ("Summit Roofing", "Residential Roofing · Portland, OR", "#6b4226"),
}

CSS = """
@page { size: Letter; margin: 0; }
body { margin: 0; font: 11.5pt "Helvetica Neue", Helvetica, sans-serif; color: #222; }
.page { padding: 0.75in 0.85in; }
.bar { height: 14pt; background: var(--c); }
.head { display: flex; align-items: center; gap: 14pt; margin-bottom: 26pt; }
.logo { width: 46pt; height: 46pt; border-radius: 50%; background: var(--c); color: #fff;
        font: bold 17pt "Helvetica Neue", sans-serif; display: flex; align-items: center; justify-content: center; }
.co { font-size: 17pt; font-weight: 700; color: var(--c); }
.addr { font-size: 9pt; color: #666; margin-top: 2pt; }
h1 { font-size: 22pt; margin: 0 0 6pt; letter-spacing: 0.01em; }
.meta { font-size: 10pt; color: #555; border-top: 1px solid #ddd; border-bottom: 1px solid #ddd;
        padding: 7pt 0; margin: 10pt 0 20pt; display: flex; gap: 28pt; }
.meta b { color: #222; }
.body { font-size: 12.5pt; line-height: 1.55; }
.total { margin-top: 26pt; margin-left: auto; width: 46%; border: 2px solid var(--c); border-radius: 6pt;
         padding: 10pt 14pt; display: flex; justify-content: space-between; font-size: 13pt; }
.total b { font-size: 15pt; }
.lines { margin-top: 30pt; }
.lines div { height: 9pt; background: #eee; border-radius: 3pt; margin: 9pt 0; }
.foot { position: fixed; bottom: 0.6in; left: 0.85in; right: 0.85in; font-size: 8.5pt; color: #888;
        border-top: 1px solid #ddd; padding-top: 6pt; }
"""

lib = Path(sys.argv[1])
out = Path(sys.argv[2])
out.mkdir(parents=True, exist_ok=True)
for line in sys.stdin:
    name, _, text = line.rstrip("\n").partition("\t")
    stem = name[:-4]
    m = re.match(r"(\d{4}-\d{2}-\d{2}) (.*)", stem)
    date, key = (m.group(1), m.group(2)) if m else ("", stem)
    issuer, addr, color = ISSUERS[key]
    parts = [p.strip() for p in text.split("⏎")]
    title, body = parts[0], " ".join(parts[1:])
    initials = "".join(w[0] for w in issuer.split() if w[0].isalpha())[:2]
    amount = re.search(r"(?:Total|amount due|Amount due|reimbursed[^,]*,|premium|installment)\s*([\d,]+\.\d\d) USD", body)
    total = (
        f'<div class="total"><span>{"Amount due" if "due" in body else "Total"}</span><b>${amount.group(1)}</b></div>'
        if amount else ""
    )
    ref = f"{abs(hash(stem)) % 900000 + 100000}"
    when = f"<span><b>Date</b> {date}</span>" if date else ""
    page = f"""<!doctype html><html><head><meta charset="utf-8"><style>{CSS}</style></head>
<body style="--c:{color}"><div class="bar"></div><div class="page">
<div class="head"><div class="logo">{html.escape(initials)}</div>
<div><div class="co">{html.escape(issuer)}</div><div class="addr">{html.escape(addr)}</div></div></div>
<h1>{html.escape(title.title() if title.isupper() else title)}</h1>
<div class="meta">{when}<span><b>Reference</b> {ref}</span><span><b>Prepared for</b> Alex Morgan</span></div>
<div class="body">{html.escape(body)}</div>
{total}
<div class="lines"><div style="width:92%"></div><div style="width:84%"></div><div style="width:88%"></div><div style="width:60%"></div></div>
<div class="foot">{html.escape(issuer)} · Please keep this document for your records.</div>
</div></body></html>"""
    (out / f"{stem}.html").write_text(page)
    print(f"{stem}")
