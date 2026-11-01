#!/usr/bin/env python3
"""把批量抓取结果拆成「每本书一个 JSON」，并生成 manifest。

app 的书库支持导入任意语料 JSON，所以每本书单独一个文件，用户逐本导入即可。

用法:
    python3 scripts/split_corpus.py --in /tmp/corpus/all.json --out ../拾遗语料/
"""
import argparse
import json
import os
import re
from collections import OrderedDict

REQUIRED = ("id", "text", "source", "book", "chapter")


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--in", dest="src", required=True)
    ap.add_argument("--out", dest="out", required=True)
    args = ap.parse_args()

    with open(args.src, encoding="utf-8") as fh:
        rows = json.load(fh)

    books = OrderedDict()
    for r in rows:
        books.setdefault(r["book"], []).append(r)

    os.makedirs(args.out, exist_ok=True)
    manifest = {}
    report = []

    for book, items in books.items():
        prefix = items[0]["id"].split("-")[0]
        fname = f"{prefix}.json"
        path = os.path.join(args.out, fname)
        with open(path, "w", encoding="utf-8") as fh:
            json.dump(items, fh, ensure_ascii=False, indent=2)

        # 自检：字段完整、id 唯一、无空正文
        ids = set()
        bad = 0
        for it in items:
            for f in REQUIRED:
                if not it.get(f):
                    bad += 1
                    break
            if it["id"] in ids:
                bad += 1
            ids.add(it["id"])
        manifest[prefix] = {"book": book, "count": len(items)}

        chapters = len({it["chapter"] for it in items})
        lens = sorted(len(it["text"]) for it in items)
        report.append((book, fname, len(items), chapters, lens[0], lens[len(lens) // 2], lens[-1], bad))

    with open(os.path.join(args.out, "manifest.json"), "w", encoding="utf-8") as fh:
        json.dump(manifest, fh, ensure_ascii=False, indent=2)

    print(f"{'书名':<12}{'文件':<14}{'条数':>5}{'篇章':>5}{'最短':>5}{'中位':>5}{'最长':>5}{'异常':>5}")
    print("-" * 60)
    for book, fname, n, ch, lo, mid, hi, bad in report:
        flag = "!!" if bad else "OK"
        print(f"{book:<12}{fname:<14}{n:>5}{ch:>5}{lo:>5}{mid:>5}{hi:>5}{flag:>4} {bad if bad else ''}")

    print(f"\n共 {len(books)} 本，{len(rows)} 条 -> {args.out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
