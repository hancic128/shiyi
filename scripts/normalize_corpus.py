#!/usr/bin/env python3
"""把抓来的语料归一到《拾遗》的排版风格。

风格基线取自 app 内置的 265 条语料（道德经 / 论语 / 庄子 / 孟子 / 传习录），
实测得到的硬约束：

  - 正文不含括号（内置 265 条里一个都没有）——括号里是校勘注、音释，
    属于笺注不是正文
  - 不出现「……」「..」「．」这类抓取残留
  - 句首不出现光杆的「曰：」「云：」——原文是「何谓量权？曰：“…”」，
    引号剥掉后就剩个悬空的「曰：」，读起来像截断
  - 不出现连续重复的标点
  - 只用全角标点

用法:
    python3 scripts/normalize_corpus.py --in /tmp/corpus/rest.json --out /tmp/corpus/rest.norm.json
    python3 scripts/normalize_corpus.py --in handwritten/rjch.json --out handwritten/rjch.json
"""
import argparse
import json
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from fetch_quotes import to_simplified  # noqa: E402

# 句首光杆的「曰：」「云：」——说话人姓名被切在上一句里了
BARE_SAY = re.compile(r"^[曰云]：")
# 括号（含全角半角方括号圆头括号）及其内容
PAREN = re.compile(r"[（(［\[【〔][^（()［\[【〔）)］\]】〕]{0,40}[)）］\]】〕]")
# 句首的说话人标记，如「[岐伯曰]气有多少…」
LEAD_MARK = re.compile(r"^[\[［【〔][^\]］】〕]{0,8}[\]］】〕]")
# 私用区字符：源站生僻字缺字形时留下的乱码（U+E000–U+F8FF）
PUA = re.compile(r"[-\U000F0000-\U000FFFFD]")
# 抓取残留
ELLIPSIS = re.compile(r"…+|\.{2,}")
# 全角句点「．」在这批书里就是句号
FULLDOT = re.compile(r"．")
# 重复标点
DUP_PUNCT = re.compile(r"([，。；：？！、])\1+")


def normalize(text: str) -> str | None:
    """归一化一条正文。无法归一（不可用）时返回 None。"""
    text = to_simplified(text)

    # 括号里的笺注不是正文；内容太长说明整条都是注，直接弃用
    def drop_paren(m: re.Match) -> str:
        inner = m.group(0)[1:-1]
        if len(inner) > 12:
            return "\x00"  # 标记为不可用
        return ""

    text = PAREN.sub(drop_paren, text)
    if "\x00" in text:
        return None

    # 以括号开头 = 整条都是笺注（「（医部全录注曰：此处必缺…」）
    if re.match(r"^[（(［\[【〔]", text):
        return None

    # 括号不配对的残渣：源文本里单独飘出来的括号字符
    text = re.sub(r"[（(［\[【〔）)］\]】〕]", "", text)

    # 句首的说话人标记「[岐伯曰]」——剥掉标记，保留正文
    text = LEAD_MARK.sub("", text)

    text = ELLIPSIS.sub("……", text)
    text = FULLDOT.sub("。", text)
    text = DUP_PUNCT.sub(r"\1", text)
    text = re.sub(r"\s+", "", text)
    text = text.strip("：，、；")

    # 私用区乱码：源站缺字形留下的标记，这类条目不能用
    if PUA.search(text):
        return None

    if not text:
        return None
    # 剥引号后剩下的光杆「曰：」
    if BARE_SAY.match(text):
        return None
    if not (4 <= len(text) <= 90):
        return None
    if text[-1] not in "。？！":
        return None
    return text


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--in", dest="src", required=True)
    ap.add_argument("--out", dest="out", required=True)
    args = ap.parse_args()

    with open(args.src, encoding="utf-8") as fh:
        rows = json.load(fh)

    out = []
    dropped = 0
    seen = set()
    dup = 0
    for q in rows:
        text = normalize(q.get("text", ""))
        if text is None:
            dropped += 1
            continue
        if text in seen:
            dup += 1
            continue
        seen.add(text)
        item = dict(q)
        item["text"] = text
        out.append(item)

    # 重排 id，保证连续且前缀不变
    by_book = {}
    for q in out:
        by_book.setdefault(q["book"], []).append(q)
    final = []
    for book, items in by_book.items():
        prefix = items[0]["id"].split("-")[0]
        for i, item in enumerate(items, 1):
            item["id"] = f"{prefix}-{i}"
        final.extend(items)

    os.makedirs(os.path.dirname(os.path.abspath(args.out)), exist_ok=True)
    with open(args.out, "w", encoding="utf-8") as fh:
        json.dump(final, fh, ensure_ascii=False, indent=2)

    print(f"{args.src} → {args.out}")
    print(f"  {len(rows)} 条 → {len(final)} 条（丢弃 {dropped}，去重 {dup}）")
    return 0


if __name__ == "__main__":
    sys.exit(main())
