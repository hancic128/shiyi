#!/usr/bin/env python3
"""从「原文 / 译文 / 注释」分栏的站点提取语录。

这类站点（如 gsw6.com）每则语录一个页面，正文里用【原文】【译文】【注释】
等方括号标签分段。原文本��标点完整，适合做卡片。

用法:
    python3 scripts/extract_sections.py --index https://www.gsw6.com/guwen/wlyh3/ \
        --book 围炉夜话 --prefix wlyh --target 120 --out handwritten/wlyh.json
"""
import argparse
import html
import json
import os
import re
import sys
import time
import urllib.request

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from fetch_quotes import clean, is_self_contained, to_simplified  # noqa: E402

UA = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36"


def fetch(url: str) -> str:
    req = urllib.request.Request(url, headers={"User-Agent": UA})
    for attempt in range(3):
        try:
            with urllib.request.urlopen(req, timeout=25) as resp:
                return resp.read().decode("utf-8", "ignore")
        except Exception:  # noqa: BLE001
            if attempt == 2:
                raise
            time.sleep(3)
    return ""


def page_lines(page: str) -> list[str]:
    """取页面正文区（contson 之类）的行。"""
    match = re.search(r'<div class="contson".*?</div>', page, flags=re.S)
    body = match.group(0) if match else page
    body = re.sub(r"<br\s*/?>", "\n", body)
    body = re.sub(r"</p>", "\n", body)
    body = re.sub(r"<[^>]+>", "", body)
    body = html.unescape(body)
    return [l.strip() for l in body.split("\n") if l.strip()]


def index_urls(index_url: str) -> list[str]:
    """从目录页取出所有条目页 URL（按页面内出现的顺序去重）。

    目录页里还混着「成语结构」之类的外链，只保留同目录下的条目页。
    """
    page = fetch(index_url)
    base = index_url.rsplit("/", 1)[0]
    seen = set()
    urls = []
    for href in re.findall(r'href="([^"]+)"', page):
        if not href.endswith(".html"):
            continue
        if href.startswith("/"):
            href = index_url.split("/guwen/")[0] + href
        elif not href.startswith("http"):
            href = f"{base}/{href}"
        # 只保留与目录页同前缀的条目页，滤掉成语结构等外链
        if not href.startswith(base + "/"):
            continue
        if href in seen:
            continue
        seen.add(href)
        urls.append(href)
    return urls


def original_text(lines: list[str]) -> tuple[str, str]:
    """返回 (小节名, 原文)。原文取【原文】到下一个【…】之间。"""
    title = ""
    for line in lines:
        if "则" in line and "：" in line:
            title = line
            break
    try:
        start = lines.index("【原文】")
    except ValueError:
        return title, ""

    chunks = []
    for line in lines[start + 1:]:
        if line.startswith("【"):
            break
        chunks.append(line)
    return title, "".join(chunks)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--index", required=True, help="目录页 URL")
    ap.add_argument("--book", required=True)
    ap.add_argument("--prefix", required=True)
    ap.add_argument("--target", type=int, default=120)
    ap.add_argument("--out", required=True)
    args = ap.parse_args()

    urls = index_urls(args.index)
    print(f"条目页 {len(urls)} 个", flush=True)

    rows = []
    seen = set()
    for url in urls:
        try:
            page = fetch(url)
        except Exception as exc:  # noqa: BLE001
            print(f"  ✗ {url}: {exc}", flush=True)
            continue
        title, raw = original_text(page_lines(page))
        if not raw:
            continue
        # 小节名去掉「第一则：」前缀里的序号，保留题眼
        chapter = re.sub(r"^第[^：:]{0,6}[则條条][：:]\s*", "", title).strip()
        if not chapter:
            chapter = args.book
        chapter = to_simplified(chapter)

        for sent in re.split(r"(?<=[。！？])", raw):
            s = clean(sent)
            if not (6 <= len(s) <= 90):
                continue
            if not is_self_contained(s):
                continue
            if s in seen:
                continue
            seen.add(s)
            rows.append({"text": s, "chapter": chapter})
        time.sleep(0.35)

    rows = rows[:args.target]
    out_rows = [
        {
            "id": f"{args.prefix}-{i + 1}",
            "text": r["text"],
            "source": f"《{args.book}》",
            "book": args.book,
            "chapter": r["chapter"],
        }
        for i, r in enumerate(rows)
    ]
    os.makedirs(os.path.dirname(os.path.abspath(args.out)), exist_ok=True)
    with open(args.out, "w", encoding="utf-8") as fh:
        json.dump(out_rows, fh, ensure_ascii=False, indent=2)
    print(f"\n写入 {args.out}: {len(out_rows)} 条，{len({r['chapter'] for r in out_rows})} 节")
    for r in out_rows[:6]:
        print(f"  [{r['chapter']}] {r['text']}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
