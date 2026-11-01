#!/usr/bin/env python3
"""抓取 gushiwen.cn 上的古籍原文，切分为语录条目。

用法:
    python3 scripts/fetch_quotes.py <book_id>            # 抓取一本书，打印统计
    python3 scripts/fetch_quotes.py <book_id> --dump     # 抓取并打印全部条目

book_id 取自 gushiwen 的古籍索引页 https://www.gushiwen.cn/guwen/
"""
import argparse
import html
import json
import os
import re
import sys
import time
import urllib.request

BASE = "https://www.gushiwen.cn"
UA = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36"

# 繁简转换：部分来源（《国语》等）提供的是繁体，而现有语料统一是简体。
# opencc 是可选依赖，缺了就跳过——多数来源本来就是简体。
_T2S = None
try:
    from opencc import OpenCC

    _T2S = OpenCC("t2s")
except Exception:  # noqa: BLE001
    pass


def to_simplified(text: str) -> str:
    """繁体转简体。没有 opencc 时原样返回。"""
    if _T2S is None:
        return text
    return _T2S.convert(text)

# 正文字数区间（对标现有语料：论语/孟子/传习录中位数 16-18 字，道德经最长 90 字）
MIN_CHARS = 6
MAX_CHARS = 90

# 断句后不应出现在句首的字——出现即说明切出了半句。
# 注意：不能收录「是」「为」「而」这类常见起句虚词（「是以圣人抱一为天下式」
# 「为学日益，为道日损」都是正常开头），只收真正的连接性残渣。
BAD_HEAD = "，、；：。？！的了乎哉焉耳也则而若如且乃者乎兮矣"
# 断句后不应出现在句尾的字——出现即说明切在了中间
BAD_TAIL = "，、；："

# 对话体的引子句，单独作为语录没有意义（《黄帝内经》《难经》等大量出现）
# 形如「黄帝问曰：…」「帝曰：…」「雷公问于黄帝曰：…」
DIALOGUE_LEAD = re.compile(
    r"^(黄帝|帝|岐伯|雷公|伯高|伯曰|少师|少帝|俞师|扁鹊|黄帝曰|帝曰)[曰曰：:]"
)
DIALOGUE_ANY = re.compile(r"^(黄帝|帝|岐伯|雷公|伯高|少师|俞师|师)[问对]曰")
# 「某某问曰：」开头的引子。判据：冒号前 12 字内同时出现「问」和「曰/云」。
# 用字符集合判断而不是正则——「抱一为天下式」的「为」不该被当成「对」。
DIALOGUE_MARKERS = ("问", "曰", "云", "对")

# 「某某谏曰：」「某某谓曰：」——对话引子的另一种形态（先秦史书常见）。
# 排除「君子曰」「夫子曰」——这类是作者转述引文，属于正文。
SPEECH_LEAD = re.compile(
    r"^(?!君子曰|夫子曰|古人曰|诗曰|书曰|易曰|老子曰|孔子曰)[一-鿿]{1,6}"
    r"(谏|对|答|请|告|谓|叹|曰|言)[曰云]?："
)
# 注释性行：「经书：…」「案：…」「注：…」
ANNOTATION_LEAD = re.compile(
    r"^(经书|案|注|疏|正义|音释|笺|凡例|按|谨按|补注|校勘|阙|衍|讹|今按|原注)[:：]"
)

# 校勘注记，混在正文里但不是正文本身，例如「（生存一作：生成）」「［一作曰］」
_ANNOT_WORDS = ("一作", "亦作", "原作", "同上", "下同", "通作", "同")
ANNOTATION = re.compile(
    r"^\s*[（(［\[【][^）)］\]】]{0,40}(?:一作|亦作|原作|同上|下同|通作|同)[^）)］\]】]{0,40}[）)］\]】]\s*$"
)
# 半角括号（正文里不该出现）
HALF_PAREN = re.compile(r"[()\[\]]")


def fetch(url: str) -> str:
    """抓取页面。gushiwen 对高频请求返回 HTTP 512（限流），所以退避要拉长。"""
    req = urllib.request.Request(url, headers={"User-Agent": UA})
    delay = 5
    for attempt in range(5):
        try:
            with urllib.request.urlopen(req, timeout=25) as resp:
                return resp.read().decode("utf-8", "ignore")
        except urllib.error.HTTPError as exc:
            if exc.code not in (512, 429, 503):
                raise
            if attempt == 4:
                raise
            print(f"  限流({exc.code})，{delay}s 后重试 {attempt + 1}", file=sys.stderr)
            time.sleep(delay)
            delay *= 2
        except Exception:
            if attempt == 4:
                raise
            print(f"  重试 {attempt + 1}", file=sys.stderr)
            time.sleep(delay)
            delay *= 2
    return ""


def strip_html(raw: str) -> str:
    raw = re.sub(r"<script.*?</script>", "", raw, flags=re.S)
    raw = re.sub(r"<style.*?</style>", "", raw, flags=re.S)
    raw = re.sub(r"<br\s*/?>", "\n", raw)
    raw = re.sub(r"</p>", "\n", raw)
    raw = re.sub(r"<[^>]+>", "", raw)
    return html.unescape(raw)


def chapter_list(book_id: str) -> list[tuple[str, str]]:
    """返回 [(章节名, 章节URL)]。"""
    page = fetch(f"{BASE}/guwen/book_{book_id}.aspx")
    seen = {}
    for href, title in re.findall(r'href="(/guwen/bookv_[^"]+)"[^>]*>([^<]{0,40})</a>', page):
        title = html.unescape(title).strip()
        if title and title not in seen:
            seen[title] = href
    return list(seen.items())


def chapter_text(url: str) -> list[str]:
    """返回该章的段落列表（每行一个自然段）。"""
    page = fetch(BASE + url)
    match = re.search(r'<div class="contson".*?</div>\s*</div>', page, flags=re.S)
    body = match.group(0) if match else page
    text = strip_html(body)
    paras = []
    for line in text.split("\n"):
        line = line.replace("　", "").strip()
        if line:
            paras.append(line)
    return paras


def split_sentences(paras: list[str]) -> list[str]:
    """按句读切分。只在句末标点（。！？）断句——分号处断开会切出半句。"""
    out = []
    for para in paras:
        for piece in re.split(r"(?<=[。！？])", para):
            piece = piece.strip()
            if piece:
                out.append(piece)
    return out


def clean(text: str) -> str:
    """归一化：去空白、去掉包裹引号、统一标点。

    现有语料全部不含引号（用词如「道可道，非常道」），所以这里把成对引号剥掉，
    只保留引号内的文字，避免卡片上出现半边引号。
    """
    text = re.sub(r"\s+", "", text)
    text = to_simplified(text)
    # 剥掉成对引号（含嵌套书名号），保留内容
    text = re.sub(r"[「『《“]|[」』》”]", "", text)
    text = text.replace('"', "").replace("'", "")
    # 半角标点转全角
    text = (text.replace(",", "，").replace(";", "；").replace(":", "：")
                .replace("?", "？").replace("!", "！"))
    return text.strip()


def is_self_contained(text: str) -> bool:
    """粗筛：句子两端不该是断句残渣。"""
    if not text:
        return False
    if text[0] in BAD_HEAD:
        return False
    if text[-1] in BAD_TAIL:
        return False
    # 对话体引子单独成条没意义（「黄帝问曰：…」「岐伯对曰：…」）
    if DIALOGUE_LEAD.match(text) or DIALOGUE_ANY.match(text):
        return False
    if is_dialogue_lead(text):
        return False
    # 校勘注记不是正文
    if ANNOTATION.match(text):
        return False
    # 注释性行不是正文
    if ANNOTATION_LEAD.match(text):
        return False
    # 「某某谏曰：」这类引子单独成条没意义
    if SPEECH_LEAD.match(text):
        return False
    # 半角括号
    if HALF_PAREN.search(text):
        return False
    return True


def is_dialogue_lead(text: str) -> bool:
    """判断是不是对话体引子：「……问曰：……」这种开头。

    判据是冒号前 12 字里同时出现「问」和「曰/云」。
    用字符判断而非正则，避免「抱一为天下式」的「为」被误当成「对」。
    """
    if "：" not in text:
        return False
    head = text.split("：", 1)[0][:12]
    if "问" in head and ("曰" in head or "云" in head):
        return True
    return False


def group_by_sentence_start(paras: list[str], group_size: int = 4) -> list[tuple[str, list[str]]]:
    """把无篇章结构的书（如《三字经》《千字文》）按语义分段。

    gushiwen 上这类书只有「全文」一章，需要自己切分。
    做法：每 group_size 段一组，组名取该组首段首句的前 3-4 字——
    三字经得到「人之初」「养不教」，千字文得到「天地玄黄」这样的标记。
    """
    groups = []
    for i in range(0, len(paras), group_size):
        chunk = paras[i:i + group_size]
        first = chunk[0]
        head = re.split(r"[。！？，、；]", first)[0].strip()
        # 三字经首句是 3 字，千字文是 4 字；取到 6 字以内足够辨识
        label = head[:6] if len(head) > 6 else head
        if not label:
            label = f"第{i // group_size + 1}段"
        groups.append((label, chunk))
    return groups


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("book_id")
    ap.add_argument("--dump", action="store_true")
    ap.add_argument("--min", type=int, default=MIN_CHARS)
    ap.add_argument("--max", type=int, default=MAX_CHARS)
    args = ap.parse_args()

    chapters = chapter_list(args.book_id)
    print(f"章节 {len(chapters)} 个")

    rows = []
    for title, url in chapters:
        paras = chapter_text(url)
        for sent in split_sentences(paras):
            sent = clean(sent)
            if not (args.min <= len(sent) <= args.max):
                continue
            if not is_self_contained(sent):
                continue
            rows.append({"chapter": title, "text": sent})
        print(f"  {title}: 累计 {len(rows)} 条")
        time.sleep(0.4)

    print(f"\n合计 {len(rows)} 条（{args.min}-{args.max} 字）")
    if args.dump:
        for r in rows:
            print(f"[{r['chapter']}] {r['text']}")
    else:
        print(json.dumps(rows[:3], ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    sys.exit(main())
