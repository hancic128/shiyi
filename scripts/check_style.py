#!/usr/bin/env python3
"""检查语料是否符合《拾遗》的排版风格。

风格基线取自现有内置语料（道德经/论语/庄子/孟子/传习录 共 265 条）：
  - 只用全角标点，无半角逗号分号冒号问号叹号
  - 正文不含任何引号（“ ”「 」『 』）与书名号《 》
  - 每条以句末标点（。？！）结尾
  - 无连续空格、无首尾空白
  - 长度 4-90 字

用法:
    python3 scripts/check_style.py src-tauri/quotes/*.json
    python3 scripts/check_style.py --dir /path/to/语料
"""
import argparse
import json
import os
import re
import sys

QUOTE_MARKS = "“”「」『』《》\"'"
HALF_PUNCT = re.compile(r"[,;:?!]")
# 括号：内置 265 条里一个都没有——括号里是校勘注、音释，属笺注不是正文
BRACKETS = re.compile(r"[（）()\[\]【】〔〕]")
# 私用区字符：源站生僻字缺字形时留下的乱码
PUA = re.compile(r"[-\U000F0000-\U000FFFFD]")
SENT_END = "。？！"


def check_file(path: str) -> tuple[list[str], int]:
    errors = []
    with open(path, encoding="utf-8") as fh:
        data = json.load(fh)

    ids = set()
    for idx, q in enumerate(data):
        where = f"{os.path.basename(path)}[{idx}] ({q.get('id', '无id')})"
        text = q.get("text", "")
        if not text:
            errors.append(f"{where}: 正文为空")
            continue
        if q.get("id") in ids:
            errors.append(f"{where}: id 重复")
        ids.add(q.get("id"))
        if text != text.strip():
            errors.append(f"{where}: 正文有首尾空白")
        if "  " in text or "\t" in text:
            errors.append(f"{where}: 正文含连续空格或制表符")
        if HALF_PUNCT.search(text):
            errors.append(f"{where}: 含半角标点 {HALF_PUNCT.search(text).group()!r}")
        marks = [m for m in QUOTE_MARKS if m in text]
        if marks:
            errors.append(f"{where}: 含引号/书名号 {''.join(marks)!r}")
        if BRACKETS.search(text):
            errors.append(f"{where}: 含括号 {BRACKETS.search(text).group()!r}")
        if PUA.search(text):
            errors.append(f"{where}: 含私用区乱码字符")
        if text[-1] not in SENT_END:
            errors.append(f"{where}: 未以句末标点结尾（末字 {text[-1]!r}）")
        if not (4 <= len(text) <= 90):
            errors.append(f"{where}: 长度 {len(text)} 超出 4-90")
        for f in ("id", "source", "book", "chapter"):
            if not q.get(f):
                errors.append(f"{where}: 缺少字段 {f}")
        if q.get("source") != f"《{q.get('book')}》":
            errors.append(f"{where}: source 与 book 不一致")
    return errors, len(data)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("files", nargs="*")
    ap.add_argument("--dir")
    args = ap.parse_args()

    paths = list(args.files)
    if args.dir:
        paths += [
            os.path.join(args.dir, f)
            for f in sorted(os.listdir(args.dir))
            if f.endswith(".json") and f != "manifest.json"
        ]
    if not paths:
        print("没有指定文件")
        return 1

    total_err = 0
    total_quotes = 0
    for path in paths:
        if not os.path.exists(path):
            print(f"跳过（不存在）: {path}")
            continue
        errors, n = check_file(path)
        total_err += len(errors)
        total_quotes += n
        flag = "OK " if not errors else "!! "
        print(f"{flag}{os.path.basename(path):<16}{n:>5} 条" + (f"  {len(errors)} 处问题" if errors else ""))
        for e in errors[:10]:
            print(f"    ✗ {e}")
        if len(errors) > 10:
            print(f"    ... 另有 {len(errors) - 10} 处")

    print(f"\n合计 {total_quotes} 条，{total_err} 处问题")
    return 1 if total_err else 0


if __name__ == "__main__":
    sys.exit(main())
