#!/usr/bin/env python3
"""从静态公版古籍页面提取语录，生成 app 语料。

用于 gushiwen 之外的来源（九五查询、太玄书阁等），这些站点不提供
gushiwen 那样的章节 URL，需要从整页正文里切分。

用法:
    python3 scripts/extract_page.py --url URL --book 书名 --prefix 前缀 \
        --start-marker "昔时贤文" [--group-size 4] --out out.json
"""
import argparse
import html
import json
import os
import re
import sys
import urllib.request

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from fetch_quotes import clean, is_self_contained, group_by_sentence_start  # noqa: E402

UA = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36"

# 页面导航/页脚等非正文行——这些不是古籍原文
NOISE = re.compile(
    r"(古籍在线阅读|九五查询|95cx|首页|上一[页章]|下一[页章]|返回目录|"
    r"收藏本站|字体|背景|打印|分享|举报|加入书架|推荐阅读|完本小说)"
)


def fetch(url: str) -> str:
    req = urllib.request.Request(url, headers={"User-Agent": UA})
    with urllib.request.urlopen(req, timeout=25) as resp:
        return resp.read().decode("utf-8", "ignore")


def extract_lines(raw: str, start_marker: str | None) -> list[str]:
    """剥 HTML，保留段落行。"""
    raw = re.sub(r"<script.*?</script>", "", raw, flags=re.S)
    raw = re.sub(r"<style.*?</style>", "", raw, flags=re.S)
    raw = re.sub(r"<br\s*/?>", "\n", raw)
    raw = re.sub(r"</p>", "\n", raw)
    raw = re.sub(r"</div>", "\n", raw)
    raw = re.sub(r"<[^>]+>", "", raw)
    raw = html.unescape(raw)
    lines = [l.strip() for l in raw.split("\n") if l.strip()]
    # 去掉导航、页脚等非正文行
    lines = [l for l in lines if not NOISE.search(l)]

    if start_marker:
        for i, l in enumerate(lines):
            if start_marker in l:
                # 把含标记的行也保留（标记可能在行首）
                lines = lines[i:]
                break
    return lines


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--url", required=True)
    ap.add_argument("--book", required=True)
    ap.add_argument("--prefix", required=True)
    ap.add_argument("--start-marker")
    ap.add_argument("--end-marker")
    ap.add_argument("--group-size", type=int, default=4)
    ap.add_argument("--target", type=int, default=120)
    ap.add_argument("--out", required=True)
    args = ap.parse_args()

    raw = fetch(args.url)
    lines = extract_lines(raw, args.start_marker)
    if args.end_marker:
        for i, l in enumerate(lines):
            if args.end_marker in l:
                lines = lines[:i]
                break

    # 按语义分组：每 group_size 行一组，组名取首行开头
    groups = group_by_sentence_start(lines, args.group_size)

    rows = []
    seen = set()
    for label, chunk in groups:
        for line in chunk:
            for sent in re.split(r"(?<=[。！？])", line):
                s = clean(sent)
                if not (6 <= len(s) <= 90):
                    continue
                if not is_self_contained(s):
                    continue
                if s in seen:
                    continue
                seen.add(s)
                rows.append({
                    "id": f"{args.prefix}-{len(rows) + 1}",
                    "text": s,
                    "source": f"《{args.book}》",
                    "book": args.book,
                    "chapter": label,
                })

    rows = rows[:args.target]
    with open(args.out, "w", encoding="utf-8") as fh:
        json.dump(rows, fh, ensure_ascii=False, indent=2)
    print(f"写入 {args.out}: {len(rows)} 条，{len({r['chapter'] for r in rows})} 组")
    for r in rows[:5]:
        print(f"  [{r['chapter']}] {r['text']}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
