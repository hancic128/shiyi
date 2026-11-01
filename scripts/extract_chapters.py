#!/usr/bin/env python3
"""从九五查询（guji.95cx.com）的分章页面提取语录，生成 app 语料。

正文在 <article class="guji-content"> 里，比整页剥离干净得多——
导航、页脚、阅读工具栏都��会混进来。

用法:
    python3 scripts/extract_chapters.py --slug xiaochuangyouji --book 小窗幽记 \
        --prefix xxyj --target 120 --out handwritten/xxyj.json
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

BASE = "https://guji.95cx.com"
UA = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36"

# 导航/页脚等非正文行
NOISE = re.compile(
    r"(古籍在线阅读|九五查询|95cx|上一[页章]|下一[页章]|返回目录|收藏本站|"
    r"字体|背景|打印|分享|举报|加入书架|推荐阅读|完本小说|造句|近义词|反义词|"
    r"节日大全|古籍史书|民俗文化|日常工具|学习教育|首页|免责说明|"
    r"Copyright|工具箱|实用工具|更多实用工具|汉字转拼音|科学计算器|日期计算器|"
    r"编码解码|页面留言|当前页面|联系方式|留言内容|取消|提交留言|二维码|"
    r"使用微信|扫一扫|浏览或相机|上一页|下一页|字体大小|朗读|语速|竖排|繁体|"
    r"拼音对照|复制内容|进度|目录|A\+|A-|慢速|常速|快速)"
)
# 「国语左丘明第1章」这类标题行——作者名 + 第 N 章
TITLE_LINE = re.compile(r"^[^，。：]{0,20}第\s*\d+\s*章$")
# 章节内的条目标题行：含夹注编号〔一〕、全角数字，或以「XX上１」形式开头
# 例：「周语上１穆王將征犬戎〔一〕，祭公謀父諫曰：不可〔二〕。」
ITEM_HEAD = re.compile(r"[〔［\[【]|[０-９]")
# 「§1。」「01一」这类编号行——人间词话用来标节号
SECTION_NO = re.compile(r"^(§\s*\d+[。．]?|\d{2}\s*[一二三四五六七八九十百]{1,3})$")

# 各书的序 / 跋 / 目录章节——不是正文，卡片里不该出现
SKIP_CHAPTERS = {
    "renjiancihua": r"^(正文|第一部分|第二部分|第三部分|附录|后序|序)",
    "youmengying": r"^(幽梦影序|幽梦影跋)",
    "xianqingouji": r"^(序|序例)",
    "shenyinlvyu": r"^(序|跋)",
}


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


def chapter_list(slug: str) -> list[tuple[str, str]]:
    """从目录页读出 [(章节名, URL)]。"""
    page = fetch(f"{BASE}/shu/{slug}/1")
    seen = {}
    for href, title in re.findall(rf'href="(/shu/{slug}/[^"]+)"[^>]*>([^<]{{2,40}})</a>', page):
        title = html.unescape(title).strip()
        if not title or title in seen:
            continue
        if NOISE.search(title) or title in ("目录",):
            continue
        if title.startswith("下一章：") or title.startswith("上一章："):
            continue
        if TITLE_LINE.match(title):
            continue
        seen[title] = BASE + href
    return list(seen.items())


def chapter_text(url: str) -> list[str]:
    """取该章正文行。"""
    page = fetch(url)
    match = re.search(r'<article class="guji-content".*?</article>', page, flags=re.S)
    body = match.group(0) if match else page
    body = re.sub(r"<br\s*/?>", "\n", body)
    body = re.sub(r"</p>", "\n", body)
    body = re.sub(r"<[^>]+>", "", body)
    body = html.unescape(body)
    lines = [l.strip() for l in body.split("\n") if l.strip()]
    return [
        l for l in lines
        if not NOISE.search(l) and not TITLE_LINE.match(l) and not ITEM_HEAD.search(l)
    ]


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--slug", required=True)
    ap.add_argument("--book", required=True)
    ap.add_argument("--prefix", required=True)
    ap.add_argument("--target", type=int, default=120)
    ap.add_argument("--out", required=True)
    ap.add_argument("--keep-sections", action="store_true", help="保留「§N。」这类编号行")
    args = ap.parse_args()

    skip = re.compile(SKIP_CHAPTERS.get(args.slug, r"$^"))
    chapters = [
        (t, u) for t, u in chapter_list(args.slug)
        if not skip.match(t)
    ]
    print(f"章节 {len(chapters)} 个", flush=True)

    rows = []
    seen = set()
    per_chapter = []
    for title, url in chapters:
        chapter = re.sub(r"\s+", "", title)
        chapter = chapter.replace("　", "")
        chapter = to_simplified(chapter)
        cands = []
        for line in chapter_text(url):
            if not args.keep_sections and SECTION_NO.match(line):
                continue
            for sent in re.split(r"(?<=[。！？])", line):
                s = clean(sent)
                if not (6 <= len(s) <= 90):
                    continue
                if not is_self_contained(s):
                    continue
                if s in seen:
                    continue
                seen.add(s)
                cands.append({"chapter": chapter, "text": s})
        per_chapter.append(cands)
        print(f"  {chapter}: {len(cands)} 条候选", flush=True)
        time.sleep(0.8)

    # 按章轮转取样，避免第一章刷屏——各章条数均衡
    for depth in range(max((len(c) for c in per_chapter), default=0)):
        for cands in per_chapter:
            if depth < len(cands):
                rows.append(cands[depth])
                if len(rows) >= args.target:
                    break
        if len(rows) >= args.target:
            break

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
    with open(args.out, "w", encoding="utf-8") as fh:
        json.dump(out_rows, fh, ensure_ascii=False, indent=2)
    print(f"\n写入 {args.out}: {len(out_rows)} 条，{len({r['chapter'] for r in out_rows})} 章")
    for r in out_rows[:4]:
        print(f"  [{r['chapter']}] {r['text']}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
