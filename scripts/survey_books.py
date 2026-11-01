#!/usr/bin/env python3
"""批量探测 gushiwen 古籍库：每本书的章节数与可产出的语录条数。

用途：为《拾遗》选书时，用实测数据（而非印象）判断每本书的产出规模，
以及抓取管线是否可用。

用法: python3 scripts/survey_books.py
"""
import json
import sys
import time
from concurrent.futures import ThreadPoolExecutor

sys.path.insert(0, __file__.rsplit("/", 1)[0])
from fetch_quotes import chapter_list, chapter_text, split_sentences, clean, is_self_contained  # noqa: E402

BOOKS = {
    # 先秦诸子
    "荀子": "e8bf28de6a4b",
    "墨子": "1a02b10fe543",
    "韩非子": "3b6401758902",
    "列子": "69c502d5c006",
    "淮南子": "bfcc9d090ed5",
    "吕氏春秋": "b59f91268b84",
    "鬼谷子": "d9e9f00fa83c",
    "管子": "4660f958c5cf",
    "商君书": "fcb502e3f55d",
    "公孙龙子": "3f3482c75152",
    "孔子家语": "e09e2b5f90fe",
    "尉缭子": "136db32cf48a",
    # 兵法
    "孙子兵法": "c412f1d0ea81",
    "吴子": "3f426dfee2f8",
    "司马法": "1695504e3fd0",
    "六韬": "1a363b21b83a",
    "素书": "77cf7aed4ca5",
    "孙膑兵法": "f053f6df717d",
    "将苑": "f85cbdb83bfd",
    "三十六计": "e11903fd10cd",
    "百战奇略": "4fa14519b950",
    # 经学
    "诗经(易传)": "adb08001c74f",
    "尚书": "e4ecba4b9fc4",
    "礼记": "09514c5b00f8",
    "孝经": "d0e63da87d55",
    "左传": "5138229e537e",
    "黄帝内经": "1bf1210e9d1f",
    "尔雅": "",
    # 史传
    "史记": "7723bfd24ca1",
    "汉书": "c6a0e66fd254",
    "后汉书": "62efc075fe05",
    "国语": "",
    "战国策": "f78a4fd3134a",
    "资治通鉴": "b30d262ad60d",
    "三国志": "e905beeb5064",
    "世说新语": "bbf0d02a401c",
    "搜神记": "f18dc4b6b06d",
    # 蒙学
    "三字经": "ca02032549eb",
    "千字文": "dee36f72cecb",
    "弟子规": "646551f2777c",
    "声律启蒙": "1b2874f763ff",
    "百家姓": "e5f673a3c14d",
    # 笔记家训
    "菜根谭": "83c0c4f4b3a1",
    "颜氏家训": "e4d81a7d60bd",
    "了凡四训": "f4b9d6ab25d0",
    "梦溪笔谈": "076e2373fc61",
    "智囊": "16865711ef5f",
    "论衡": "75da9969dfbc",
    # 文论诗论
    "文心雕龙": "5b87255cd674",
    # 术数相术
    "冰鉴": "74e96e4ead90",
    "山海经": "4e6b88d8a0bc",
    "伤寒论": "82865fcafa01",
    # 其他
    "大学": "758ff5e03b31",
    "中庸": "5d65674dd97e",
}


def survey(item):
    name, bid = item
    if not bid:
        return name, None, None, "无 ID"
    try:
        chapters = chapter_list(bid)
        if not chapters:
            return name, 0, 0, "无分章"
        total = 0
        for _t, url in chapters:
            paras = chapter_text(url)
            for sent in split_sentences(paras):
                s = clean(sent)
                if 6 <= len(s) <= 90 and is_self_contained(s):
                    total += 1
            time.sleep(1.2)
        return name, len(chapters), total, "OK"
    except Exception as exc:  # noqa: BLE001
        return name, None, None, f"失败: {exc}"


def main():
    items = list(BOOKS.items())
    # gushiwen 限流较严，并发只给 2
    with ThreadPoolExecutor(max_workers=2) as pool:
        results = list(pool.map(survey, items))

    ok = [r for r in results if r[3] == "OK"]
    ok.sort(key=lambda r: -(r[2] or 0))
    print(f"{'书名':<14}{'章节':>5}{'可产出':>8}   备注")
    print("-" * 46)
    for name, nch, total, status in ok:
        print(f"{name:<14}{nch:>5}{total:>8}")
    print("-" * 46)
    for name, nch, total, status in results:
        if status != "OK":
            print(f"{name:<14}{str(nch or '-'):>5}{str(total or '-'):>8}   {status}")
    print(f"\n可用 {len(ok)}/{len(items)} 本")


if __name__ == "__main__":
    main()
