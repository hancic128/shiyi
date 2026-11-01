#!/usr/bin/env python3
"""从九五查询提取《人间词话》正文。

人间词话在源站是「§1。/ 01一 / 正文…」的结构：§1 对应正文 64 节，
§2 对应删稿 49 节。本脚本按节号切分，用「第 N 则」作章节名。

三类行要排除：
  - 引文（以引号开头）：「泪眼问花花不语…」——是王��维引的别人的词，
    不是他的论述，混进去会误标出处。
  - 夹注（以（１）开头）：「（１）此为龚自珍《乙亥杂诗》…」
  - 校勘字（〔…〕）：「薄幸〔幸加单人〕」

用法:
    python3 scripts/extract_cihua.py --out handwritten/rjch.json --target 110
"""
import argparse
import html
import json
import os
import re
import sys
import urllib.request

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from fetch_quotes import clean, is_self_contained, to_simplified  # noqa: E402

UA = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36"
URL = "https://guji.95cx.com/shu/renjiancihua/1"

# 节号行：「01一」「13十三」，也可能后面直接跟着正文「03三 有有我之境，有无我之境。」
SECTION_HEAD = re.compile(r"^(\d{2})\s*([一二三四五六七八九十百]{1,3})(?:\s+(.*))?$")
# 「§1。」「§2。」——部分的标记
PART_MARK = re.compile(r"^§\s*\d+[。．]?$")
# 以引号开头 = 引文
CITED = re.compile(r"^[“「『]")
# 夹注行
ANNOT_LINE = re.compile(r"^[（(]\s*[０-９0-9]")


def fetch(url: str) -> str:
    req = urllib.request.Request(url, headers={"User-Agent": UA})
    for attempt in range(3):
        try:
            with urllib.request.urlopen(req, timeout=25) as resp:
                return resp.read().decode("utf-8", "ignore")
        except Exception:  # noqa: BLE001
            if attempt == 2:
                raise
            import time
            time.sleep(3)
    return ""


def content_lines(url: str) -> list[str]:
    page = fetch(url)
    match = re.search(r'<article class="guji-content".*?</article>', page, flags=re.S)
    body = match.group(0) if match else page
    body = re.sub(r"<br\s*/?>", "\n", body)
    body = re.sub(r"</p>", "\n", body)
    body = re.sub(r"<[^>]+>", "", body)
    body = html.unescape(body)
    return [l.strip() for l in body.split("\n") if l.strip()]


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--target", type=int, default=110)
    ap.add_argument("--out", required=True)
    args = ap.parse_args()

    lines = content_lines(URL)

    rows = []
    seen = set()
    part = "正文"
    section = None
    for line in lines:
        if PART_MARK.match(line):
            part = "正文" if line.startswith("§1") else "删稿"
            continue

        head = SECTION_HEAD.match(line)
        if head:
            num = int(head.group(1))
            # 删稿从 01 重新编号，用偏移区分开
            section = (f"第{head.group(2)}则" if part == "正文"
                       else f"删稿第{head.group(2)}则")
            rest = head.group(3)
            if rest:
                line = rest
            else:
                continue

        if section is None:
            continue  # 序言部分，还没进正文
        if CITED.match(line) or ANNOT_LINE.match(line):
            continue

        # 去掉校勘字〔…〕
        line = re.sub(r"〔[^〕]{0,10}〕", "", line)
        # 引号包裹的整句去掉引号后仍是引文，但夹在正文里的可以留
        for sent in re.split(r"(?<=[。！？])", line):
            s = clean(sent)
            if not (6 <= len(s) <= 90):
                continue
            if not is_self_contained(s):
                continue
            if s in seen:
                continue
            seen.add(s)
            rows.append({"text": s, "chapter": section})

    # 正文优先，不足再补删稿
    main_rows = [r for r in rows if r["chapter"].startswith("第")]
    extra = [r for r in rows if not r["chapter"].startswith("第")]
    picked = (main_rows + extra)[:args.target]

    out_rows = [
        {
            "id": f"rjch-{i + 1}",
            "text": r["text"],
            "source": "《人间词话》",
            "book": "人间词话",
            "chapter": r["chapter"],
        }
        for i, r in enumerate(picked)
    ]
    with open(args.out, "w", encoding="utf-8") as fh:
        json.dump(out_rows, fh, ensure_ascii=False, indent=2)
    print(f"写入 {args.out}: {len(out_rows)} 条，{len({r['chapter'] for r in out_rows})} 节")
    for r in out_rows[:6]:
        print(f"  [{r['chapter']}] {r['text']}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
