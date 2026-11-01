#!/usr/bin/env python3
"""校验拾遗语料：条数、id 唯一性、正文重复、字段完整性、章节归属。

期望条数由 src-tauri/quotes/manifest.json 驱动——加书只需在该文件加一条，
再放对应的 <prefix>.json，不必改本脚本。

用法: python3 scripts/validate_quotes.py
"""
import json
import os
import sys
from collections import Counter

HERE = os.path.dirname(os.path.abspath(__file__))
QUOTES_DIR = os.path.join(HERE, "..", "src-tauri", "quotes")
MANIFEST = os.path.join(QUOTES_DIR, "manifest.json")

REQUIRED = ("id", "text", "source", "book", "chapter")


def load_expected() -> dict:
    """从 manifest 读取期望值。manifest 缺失时退回旧的硬编码表。"""
    if os.path.exists(MANIFEST):
        with open(MANIFEST, encoding="utf-8") as fh:
            raw = json.load(fh)
        return {k: (v["book"], v["count"]) for k, v in raw.items()}
    return {
        "ddj": ("道德经", 79),
        "ly": ("论语", 78),
        "zz": ("庄子", 33),
        "mz": ("孟子", 40),
        "cxl": ("传习录", 35),
    }


def main() -> int:
    errors = []
    warnings = []
    all_quotes = []

    EXPECTED = load_expected()

    for prefix, (book, expected_count) in EXPECTED.items():
        path = os.path.join(QUOTES_DIR, f"{prefix}.json")
        if not os.path.exists(path):
            errors.append(f"缺少文件: {path}")
            continue

        with open(path, encoding="utf-8") as fh:
            try:
                data = json.load(fh)
            except json.JSONDecodeError as exc:
                errors.append(f"{prefix}.json JSON 解析失败: {exc}")
                continue

        if len(data) != expected_count:
            errors.append(f"{prefix}.json 条数 {len(data)} != 期望 {expected_count}")

        for idx, item in enumerate(data):
            where = f"{prefix}.json[{idx}] ({item.get('id', '无 id')})"
            for field in REQUIRED:
                if not item.get(field):
                    errors.append(f"{where}: 缺少字段 {field}")
            if item.get("book") != book:
                errors.append(f"{where}: book={item.get('book')!r} 应为 {book!r}")
            if item.get("source") != f"《{book}》":
                errors.append(f"{where}: source={item.get('source')!r} 应为 《{book}》")
            if not item.get("id", "").startswith(prefix + "-"):
                errors.append(f"{where}: id 前缀应为 {prefix}-")
            if len(item.get("text", "")) < 4:
                errors.append(f"{where}: 正文过短")
            # 标点检查：不应包含半角逗号结尾等排版错误
            if "  " in item.get("text", ""):
                errors.append(f"{where}: 正文含连续空格")
        all_quotes.extend(data)

    # id 唯一
    ids = [q["id"] for q in all_quotes]
    dup_ids = [k for k, v in Counter(ids).items() if v > 1]
    for d in dup_ids:
        errors.append(f"id 重复: {d}")

    # 正文重复（跨书允许，同书内视为错误）
    by_text = {}
    for q in all_quotes:
        by_text.setdefault(q["text"], []).append(q)
    for text, qs in by_text.items():
        if len(qs) > 1:
            same_book = len({q["book"] for q in qs}) == 1
            ids_str = ", ".join(f"{q['id']}({q['book']})" for q in qs)
            msg = f"正文重复: 「{text[:26]}…」 -> {ids_str}"
            (errors if same_book else warnings).append(msg)

    total = len(all_quotes)
    target = sum(c for _, c in EXPECTED.values())
    if total != target:
        errors.append(f"总条数 {total} != 期望 {target}")

    print(f"总计 {total} 条（期望 {target}）")
    for prefix, (book, exp) in EXPECTED.items():
        n = sum(1 for q in all_quotes if q["id"].startswith(prefix + "-"))
        flag = "OK " if n == exp else "!! "
        print(f"  {flag}{book:6} {n:3}/{exp}")

    if warnings:
        print(f"\n警告 {len(warnings)} 条（跨书同名，可能是不同文本，忽略）:")
        for w in warnings:
            print(f"  ~ {w}")

    if errors:
        print(f"\n错误 {len(errors)} 条:")
        for e in errors:
            print(f"  ✗ {e}")
        return 1

    print("\n✓ 全部校验通过")
    return 0


if __name__ == "__main__":
    sys.exit(main())
