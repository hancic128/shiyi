#!/usr/bin/env python3
"""从 gushiwen 抓取古籍原文，生成《拾遗》app 的语料 JSON。

产出格式与 src-tauri/quotes/*.json 一致：
    {"id": "<prefix>-<n>", "text": ..., "source": "《书名》", "book": "书名", "chapter": "篇章"}

用法:
    python3 scripts/build_corpus.py --book 冰鉴:74e96e4ead90 --prefix bj
    python3 scripts/build_corpus.py --batch --out src-tauri/quotes/bj.json
"""
import argparse
import json
import os
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from fetch_quotes import (  # noqa: E402
    chapter_list, chapter_text, split_sentences, clean, is_self_contained,
    group_by_sentence_start,
)

HERE = os.path.dirname(os.path.abspath(__file__))
QUOTES_DIR = os.path.join(HERE, "..", "src-tauri", "quotes")

# 书名 -> (gushiwen book_id, id 前缀, 每本目标条数)
# 每本目标条数参照现有语料的密度：论语 78 条 / 20 篇，道德经 79 条 / 81 章
TARGETS = {
    "冰鉴": ("74e96e4ead90", "bj", 130),
    "鬼谷子": ("d9e9f00fa83c", "ggz", 120),
    "素书": ("77cf7aed4ca5", "ss", 110),
    "菜根谭": ("83c0c4f4b3a1", "cgt", 120),
    "弟子规": ("646551f2777c", "dzg", 170),
    "三字经": ("ca02032549eb", "szj", 120),
    "千字文": ("dee36f72cecb", "qzw", 120),
    "荀子": ("e8bf28de6a4b", "xz", 120),
    "墨子": ("1a02b10fe543", "mz2", 120),
    "韩非子": ("3b6401758902", "hfz", 120),
    "列子": ("69c502d5c006", "lz", 130),
    "淮南子": ("bfcc9d090ed5", "hnz", 120),
    "吕氏春秋": ("b59f91268b84", "lscc", 120),
    "孙子兵法": ("c412f1d0ea81", "sqzf", 120),
    "吴子": ("3f426dfee2f8", "wz", 120),
    "六韬": ("1a363b21b83a", "lt", 120),
    "孝经": ("d0e63da87d55", "xj", 100),
    "颜氏家训": ("e4d81a7d60bd", "ysjx", 120),
    "了凡四训": ("f4b9d6ab25d0", "lfsx", 120),
    "史记": ("7723bfd24ca1", "sj", 120),
    "资治通鉴": ("b30d262ad60d", "zztj", 120),
    "世说新语": ("bbf0d02a401c", "ssxy", 120),
    "诗经": ("", "sj2", 0),           # gushiwen 未收录，改为手工撰写（见 NEEDS_OTHER_SOURCE）
    "礼记": ("09514c5b00f8", "lj", 120),
    "黄帝内经": ("1bf1210e9d1f", "hdjj", 120),
    "文心雕龙": ("5b87255cd674", "wxdl", 120),
    "山海经": ("4e6b88d8a0bc", "shs", 100),
    "梦溪笔谈": ("076e2373fc61", "mxbt", 120),
    "声律启蒙": ("1b2874f763ff", "slqm", 120),
    "三十六计": ("e11903fd10cd", "sslj", 100),
    "司马法": ("1695504e3fd0", "smf", 120),
    "大学": ("758ff5e03b31", "dx", 90),
    "中庸": ("5d65674dd97e", "zy", 110),
    "公孙龙子": ("3f3482c75152", "gplz", 120),
    "论衡": ("75da9969dfbc", "lh", 120),
    "智囊": ("16865711ef5f", "zn", 120),
    "搜神记": ("f18dc4b6b06d", "ssj", 120),
    "汉书": ("c6a0e66fd254", "hs", 120),
    "三国志": ("e905beeb5064", "sgz", 120),
    "管子": ("4660f958c5cf", "gz", 120),
    "商君书": ("fcb502e3f55d", "sjs", 80),
    "孔子家语": ("e09e2b5f90fe", "kzjy", 120),
    "尉缭子": ("136db32cf48a", "wlz", 100),
    "孙膑兵法": ("f053f6df717d", "sbzf", 100),
    "将苑": ("f85cbdb83bfd", "jy", 80),
    "百战奇略": ("4fa14519b950", "bzql", 80),
    "逸周书": ("c5b29ece47ca", "yzs", 100),
    "伤寒论": ("82865fcafa01", "shl", 130),
    # 实测补录
    "周易": ("adb08001c74f", "zydc", 120),
    "尚书": ("e4ecba4b9fc4", "ssj2", 120),
    "周礼": ("9f000cb5cebc", "zl", 100),
    "仪礼": ("f726eab9800a", "yl", 100),
    "后汉书": ("62efc075fe05", "hhs", 180),
    "笑林广记": ("9ec2c9145a8f", "xlgj", 130),
    "本草纲目": ("d809ec69c6fe", "bcgm", 130),
    "天工开物": ("289e1e12ab2a", "tgkw", 100),
    "水经注": ("938f9117e41d", "sjz2", 100),
    "续资治通鉴": ("b2b9c9d531ec", "xzztj", 100),
}

# gushiwen 未收录、需另找来源的书
NEEDS_OTHER_SOURCE = {
    "诗经": ("sj3", 120),
    "国语": ("gy", 120),
    "增广贤文": ("zgxw", 120),
    "二十四诗品": ("sysp", 100),
    "小窗幽记": ("xxyj", 100),
    "围炉夜话": ("wlyh", 100),
    "呻吟语": ("sny", 100),
    "幽梦影": ("ymy", 100),
    "闲情偶寄": ("xqyj", 100),
    "人间词话": ("rjch", 80),
}

# gushiwen 上只有「全文」一章、没有篇章结构的书：需要自己按语义分段
NEEDS_GROUPING = {"三字经", "千字文"}


def collect(book_id: str, target: int, book_name: str = "") -> list[dict]:
    """抓取一本书，按章轮转取句子，避免前几章刷屏。"""
    chapters = chapter_list(book_id)
    print(f"  章节 {len(chapters)} 个", flush=True)

    per_chapter = []
    for title, url in chapters:
        paras = chapter_text(url)
        if book_name in NEEDS_GROUPING:
            # 单章全文 → 按语义切段，每 4 段一组
            labeled = []
            for label, chunk in group_by_sentence_start(paras):
                for sent in split_sentences(chunk):
                    s = clean(sent)
                    if 6 <= len(s) <= 90 and is_self_contained(s):
                        labeled.append({"chapter": label, "text": s})
            per_chapter.append(labeled)
        else:
            cands = []
            for sent in split_sentences(paras):
                s = clean(sent)
                if 6 <= len(s) <= 90 and is_self_contained(s):
                    cands.append({"chapter": title, "text": s})
            per_chapter.append(cands)
        time.sleep(1.0)

    # 轮转取样：先每章取 1 条，再每章取第 2 条……
    picked, seen = [], set()
    for depth in range(max((len(c) for c in per_chapter), default=0)):
        for cands in per_chapter:
            if depth < len(cands):
                item = cands[depth]
                if item["text"] in seen:
                    continue
                seen.add(item["text"])
                picked.append(item)
                if len(picked) >= target:
                    return picked
    return picked


def build(book: str, book_id: str, prefix: str, target: int) -> list[dict]:
    print(f"[{book}]", flush=True)
    rows = collect(book_id, target, book)
    return [
        {
            "id": f"{prefix}-{i + 1}",
            "text": r["text"],
            "source": f"《{book}》",
            "book": book,
            "chapter": r["chapter"],
        }
        for i, r in enumerate(rows)
    ]


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--book", action="append", default=[], help="书名:book_id")
    ap.add_argument("--prefix", action="append", default=[], help="id 前缀（与 --book 同序）")
    ap.add_argument("--target", type=int, default=120)
    ap.add_argument("--out", help="输出 JSON 路径")
    ap.add_argument("--batch", action="store_true", help="跑 TARGETS 全表")
    ap.add_argument("--only", help="逗号分隔的书名，配合 --batch 只跑指定几本")
    args = ap.parse_args()

    jobs = []
    if args.batch:
        wanted = set(args.only.split(",")) if args.only else None
        for book, (bid, prefix, target) in TARGETS.items():
            if wanted and book not in wanted:
                continue
            jobs.append((book, bid, prefix, target))
    else:
        for i, spec in enumerate(args.book):
            book, bid = spec.split(":", 1)
            prefix = args.prefix[i] if i < len(args.prefix) else "bk"
            jobs.append((book, bid, prefix, args.target))

    all_rows = []
    out_path = args.out
    for book, bid, prefix, target in jobs:
        try:
            rows = build(book, bid, prefix, target)
        except Exception as exc:  # noqa: BLE001
            print(f"  ✗ {book}: {exc}", flush=True)
            continue
        print(f"  ✓ {book}: {len(rows)} 条", flush=True)
        all_rows.extend(rows)

        # 每本书抓完立刻落盘——抓取耗时长，中断不该丢掉已完成的成果
        if out_path:
            os.makedirs(os.path.dirname(os.path.abspath(out_path)), exist_ok=True)
            with open(out_path, "w", encoding="utf-8") as fh:
                json.dump(all_rows, fh, ensure_ascii=False, indent=2)

    if out_path:
        print(f"\n写入 {out_path}: {len(all_rows)} 条")
    return 0


if __name__ == "__main__":
    sys.exit(main())
