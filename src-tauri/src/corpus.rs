//! 语料库：每本书一个独立的 JSON 文件，运行时可增删。
//!
//! # 为什么要改成运行时书库
//!
//! 最初的实现把五本书用 `include_str!` 在编译期嵌进二进制。加一本书要改
//! `quotes.rs` 里的常量列表、重新编译、重新发布，门槛太高。
//!
//! 现在每本书是应用数据目录下的一个独立文件：
//!
//! ```text
//! ~/Library/Application Support/site.hancic.shiyi/books/
//!   道德经.json
//!   论语.json
//!   ...
//! ```
//!
//! - 启动时如果 `books/` 不存在，把编译期内嵌的语料**迁移**过去（内置书
//!   仍然随二进制分发，用户装完即可用）。
//! - 用户可以在设置里导入 / 导出 / 移除书籍，改完立即生效，不用重编译。
//!
//! # 文件格式
//!
//! 每本书一个 JSON 数组，元素是 `Quote`：
//!
//! ```json
//! [
//!   {
//!     "id": "ddj-1",
//!     "text": "道可道，非常道；……",
//!     "source": "《道德经》",
//!     "book": "道德经",
//!     "chapter": "第一章"
//!   }
//! ]
//! ```
//!
//! `source` 与 `book` 都由导入的文件自身携带，导入时不覆盖——用户导出的
//! 语料原样再导入应当得到同样的结果。

use crate::quotes::Quote;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

/// 书库清单。记录被移除的内置书，免得下次启动它们又回来。
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct Manifest {
    removed_builtins: Vec<String>,
}

/// 清单文件名。放在书库目录里。
const MANIFEST_FILE: &str = "library.json";

/// 一本书的元信息。列表和对话框用这个，不把几百条语录都传给前端。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BookInfo {
    /// 书名，如「道德经」。同一本书名在书库里唯一。
    pub name: String,
    /// 语录条数。
    pub count: usize,
    /// 该书的语录 id 前缀（取自第一条的 id 中 `-` 之前的部分）。
    pub prefix: String,
    /// 是否是随应用分发的内置书籍。内置书可以移除，移除后不��自动恢复
    /// （除非用户重新导入），否则「移除」这个操作就没有意义了。
    pub builtin: bool,
}

/// 导入 / 导出 / 移除的结果。`ok` 为 false 时 `message` 是给用户看的提示。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BookOpResult {
    pub ok: bool,
    pub message: String,
}

impl BookOpResult {
    /// 成功。`message` 是给用户看的中文提示。
    pub fn ok(message: impl Into<String>) -> Self {
        Self {
            ok: true,
            message: message.into(),
        }
    }

    /// 失败。`message` 说明为什么失败。
    pub fn err(message: impl Into<String>) -> Self {
        Self {
            ok: false,
            message: message.into(),
        }
    }
}

/// 内置语料。编译期嵌入，首次启动时迁移到书库目录。
///
/// 仍然用 `include_str!` 是有意为之：用户装完应用就能直接用，不需要先去
/// 某个目录放文件。迁移之后这些字节就不在磁盘上了，但二进制里留着也无妨。
pub const BUILTIN_RAW: &[(&str, &str)] = &[
    ("ddj", include_str!("../quotes/ddj.json")),
    ("ly", include_str!("../quotes/ly.json")),
    ("zz", include_str!("../quotes/zz.json")),
    ("mz", include_str!("../quotes/mz.json")),
    ("cxl", include_str!("../quotes/cxl.json")),
];

/// 语料库。所有书籍的语录都缓存在内存里，改动后整体替换。
pub struct Corpus {
    dir: PathBuf,
    inner: Mutex<Library>,
}

/// 内存里的书库：书名 -> 语录列表。保持插入顺序，列表页按导入顺序展示。
#[derive(Default)]
struct Library {
    /// 书名 -> 书籍
    books: HashMap<String, Book>,
    /// 书籍的展示顺序（书名），导入顺序。
    order: Vec<String>,
}

/// 一本书在内存里的表示。
struct Book {
    /// 语录 id 的前缀，用来给导入的新语录生成不冲突的 id。
    prefix: String,
    /// 是否内置。
    builtin: bool,
    quotes: Vec<Quote>,
}

impl Corpus {
    /// 打开书库。第一次调用会把内置语料迁移到磁盘。
    pub fn global() -> &'static Corpus {
        static CORPUS: OnceLock<Corpus> = OnceLock::new();
        CORPUS.get_or_init(|| {
            let dir = crate::paths::app_data_dir().join("books");
            let corpus = Corpus {
                dir,
                inner: Mutex::new(Library::default()),
            };
            corpus.load();
            corpus
        })
    }

    /// 读取全部书籍到内存。
    ///
    /// 顺序：
    /// 1. 读清单 `library.json`，拿到被移除的内置书名单——移除过的内置书
    ///    不会在下次启动时复活。
    /// 2. 内置语料：没被移除的，磁盘上已有文件就用文件的（用户可能改过），
    ///    没有就写一份。
    /// 3. 磁盘上其余的 JSON 文件是用户导入的书。
    fn load(&self) {
        let _ = fs::create_dir_all(&self.dir);

        let removed = self.read_removed_builtins();
        let mut library = Library::default();

        // 内置书
        for (prefix, raw) in BUILTIN_RAW {
            let Ok(items) = serde_json::from_str::<Vec<Quote>>(raw) else {
                continue;
            };
            let Some(name) = items.first().map(|q| q.book.clone()) else {
                continue;
            };
            if removed.contains(&name) {
                // 用户移除过，不复活
                continue;
            }

            // 磁盘上已有同名文件 → 以磁盘为准（用户可能编辑过内置书）
            let quotes = match self.read_book_file(&name) {
                Some(from_disk) => from_disk,
                None => {
                    let _ = self.write_book_file(&name, &items);
                    items
                }
            };
            let prefix = quotes
                .first()
                .map(|q| id_prefix(&q.id))
                .unwrap_or_else(|| (*prefix).to_string());

            library.order.push(name.clone());
            library.books.insert(
                name.clone(),
                Book {
                    prefix,
                    builtin: true,
                    quotes,
                },
            );
        }

        // 磁盘上其余的 JSON 文件是用户导入的书
        let mut user_books: Vec<(String, Book)> = Vec::new();
        if let Ok(entries) = fs::read_dir(&self.dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) != Some("json") {
                    continue;
                }
                // 清单文件不是书
                if path.file_name().and_then(|n| n.to_str()) == Some(MANIFEST_FILE) {
                    continue;
                }
                let Ok(raw) = fs::read_to_string(&path) else {
                    continue;
                };
                let Ok(items) = serde_json::from_str::<Vec<Quote>>(&raw) else {
                    continue;
                };
                let Some(name) = items.first().map(|q| q.book.clone()) else {
                    continue;
                };
                // 内置书已经在上面处理过了
                if library.books.contains_key(&name) {
                    continue;
                }
                let prefix = items
                    .first()
                    .map(|q| id_prefix(&q.id))
                    .unwrap_or_else(|| "bk".to_string());
                user_books.push((
                    name.clone(),
                    Book {
                        prefix,
                        builtin: false,
                        quotes: items,
                    },
                ));
            }
        }
        user_books.sort_by(|a, b| a.0.cmp(&b.0));
        for (name, book) in user_books {
            library.order.push(name.clone());
            library.books.insert(name, book);
        }

        *self.inner.lock().expect("corpus mutex poisoned") = library;
    }

    /// 读清单文件，记录被移除的内置书。
    fn read_removed_builtins(&self) -> HashSet<String> {
        let path = self.manifest_path();
        let Ok(raw) = fs::read_to_string(path) else {
            return HashSet::new();
        };
        serde_json::from_str::<Manifest>(&raw)
            .map(|m| m.removed_builtins.into_iter().collect())
            .unwrap_or_default()
    }

    /// 写清单文件。
    fn write_removed_builtins(&self, removed: &HashSet<String>) {
        let manifest = Manifest {
            removed_builtins: removed.iter().cloned().collect(),
        };
        let Ok(raw) = serde_json::to_string_pretty(&manifest) else {
            return;
        };
        let _ = fs::write(self.manifest_path(), raw);
    }

    /// 清单文件路径。放在书库目录里，跟着书库走。
    fn manifest_path(&self) -> PathBuf {
        self.dir.join(MANIFEST_FILE)
    }

    /// 读磁盘上某本书的语录。
    fn read_book_file(&self, name: &str) -> Option<Vec<Quote>> {
        let raw = fs::read_to_string(self.book_path(name)).ok()?;
        serde_json::from_str(&raw).ok()
    }

    /// 把一本书的语录写到磁盘。
    fn write_book_file(&self, name: &str, quotes: &[Quote]) -> std::io::Result<()> {
        let _ = fs::create_dir_all(&self.dir);
        let path = self.book_path(name);
        let raw = serde_json::to_string_pretty(quotes)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        fs::write(path, raw)
    }

    /// 书名 -> 文件路径。文件名做安全化处理，避免路径穿越。
    fn book_path(&self, name: &str) -> PathBuf {
        self.dir.join(format!("{}.json", sanitize(name)))
    }

    /// 全部语录。
    pub fn all(&self) -> Vec<Quote> {
        let library = self.inner.lock().expect("corpus mutex poisoned");
        let mut out = Vec::new();
        for name in &library.order {
            if let Some(book) = library.books.get(name) {
                out.extend(book.quotes.iter().cloned());
            }
        }
        out
    }

    /// 按 id 找一条语录。
    pub fn by_id(&self, id: &str) -> Option<Quote> {
        let library = self.inner.lock().expect("corpus mutex poisoned");
        library
            .books
            .values()
            .flat_map(|b| b.quotes.iter())
            .find(|q| q.id == id)
            .cloned()
    }

    /// 书籍列表。
    pub fn books(&self) -> Vec<BookInfo> {
        let library = self.inner.lock().expect("corpus mutex poisoned");
        library
            .order
            .iter()
            .filter_map(|name| {
                library.books.get(name).map(|book| BookInfo {
                    name: name.clone(),
                    count: book.quotes.len(),
                    prefix: book.prefix.clone(),
                    builtin: book.builtin,
                })
            })
            .collect()
    }

    /// 导入一本书。
    ///
    /// 重复导入的判定分两层：
    /// 1. **同名**：书库里已经有同名书籍 —— 直接拒绝，告诉用户「已存在」。
    /// 2. **同内容**：书名不同但内容指纹一样 —— 也拒绝，避免同一部书换个名字
    ///    塞进来导致弹出重复语录。
    ///
    /// 指纹用「全部语录 id + 正文」的哈希。同一本书如果重新导出再导入，
    /// 指纹一致，会被第二层拦住。
    pub fn import(&self, name: &str, quotes: Vec<Quote>) -> BookOpResult {
        if quotes.is_empty() {
            return BookOpResult::err("这个文件里没有语录");
        }

        // 校验：字段完整、id 唯一、正文非空。
        let mut seen_ids = std::collections::HashSet::new();
        for q in &quotes {
            if q.id.trim().is_empty() {
                return BookOpResult::err("有条语录缺少 id");
            }
            if q.text.trim().is_empty() {
                return BookOpResult::err(format!("语录 {} 缺少正文", q.id));
            }
            if q.book.trim().is_empty() {
                return BookOpResult::err(format!("语录 {} 缺少书名", q.id));
            }
            if !seen_ids.insert(q.id.clone()) {
                return BookOpResult::err(format!("语录 id 重复：{}", q.id));
            }
        }

        let mut library = self.inner.lock().expect("corpus mutex poisoned");

        // 第 1 层：同名。
        if library.books.contains_key(name) {
            return BookOpResult::err(format!("《{name}》已在书库中，未重复导入"));
        }

        // 第 2 层：内容指纹相同。
        let fingerprint = fingerprint_of(&quotes);
        for existing_name in &library.order {
            if let Some(book) = library.books.get(existing_name) {
                if fingerprint_of(&book.quotes) == fingerprint {
                    return BookOpResult::err(format!(
                        "内容与《{existing_name}》完全相同，未重复导入"
                    ));
                }
            }
        }

        // 落盘失败就整体放弃，避免内存和磁盘不一致。
        if let Err(err) = self.write_book_file(name, &quotes) {
            return BookOpResult::err(format!("写入书库失败：{err}"));
        }

        let prefix = id_prefix(&quotes[0].id);
        library.order.push(name.to_string());
        library.books.insert(
            name.to_string(),
            Book {
                prefix,
                builtin: false,
                quotes,
            },
        );
        BookOpResult::ok(format!("已导入《{name}》"))
    }

    /// 移除一本书。
    ///
    /// 同时清掉指向该书语录的收藏——不然收藏夹里会留下永远点不开的条目。
    /// 历史保留（它存的是正文快照，不依赖语录还在）。
    ///
    /// 移除内置书时会记进清单，下次启动不再复活它。
    pub fn remove(&self, name: &str) -> BookOpResult {
        let mut library = self.inner.lock().expect("corpus mutex poisoned");
        let Some(book) = library.books.remove(name) else {
            return BookOpResult::err(format!("书库里没有《{name}》"));
        };
        library.order.retain(|n| n != name);

        // 删磁盘上的文件；文件不在也不影响内存里的移除结果。
        let path = self.book_path(name);
        let _ = fs::remove_file(path);

        // 内置书被移除后要记进清单，否则下次启动会被重新迁移回来
        if book.builtin {
            let mut removed = self.read_removed_builtins();
            removed.insert(name.to_string());
            self.write_removed_builtins(&removed);
        }

        // 清理收藏
        let quote_ids: std::collections::HashSet<&str> =
            book.quotes.iter().map(|q| q.id.as_str()).collect();
        let removed_favs = crate::store::DataStore::global().remove_favorites(|id| quote_ids.contains(id));

        let mut msg = format!("已移除《{name}》");
        if removed_favs > 0 {
            msg.push_str(&format!("，同时清掉 {removed_favs} 条收藏"));
        }
        BookOpResult::ok(msg)
    }

    /// 导出一本书的语录，返回序列化后的 JSON 文本。
    pub fn export(&self, name: &str) -> Result<String, String> {
        let library = self.inner.lock().expect("corpus mutex poisoned");
        let book = library
            .books
            .get(name)
            .ok_or_else(|| format!("书库里没有《{name}》"))?;
        serde_json::to_string_pretty(&book.quotes)
            .map_err(|e| format!("导出失败：{e}"))
    }
}

/// 从语录 id 里取前缀：`ddj-12` -> `ddj`。没有 `-` 就用前 4 个字符。
fn id_prefix(id: &str) -> String {
    match id.split_once('-') {
        Some((prefix, _)) if !prefix.is_empty() => prefix.to_string(),
        _ => id.chars().take(4).collect(),
    }
}

/// 语录内容的指纹：把 id 和正文拼起来算哈希。
///
/// 只用 id + 正文，不含书名——这样「同一部书改了名字」也能被认出来。
fn fingerprint_of(quotes: &[Quote]) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    for q in quotes {
        q.id.hash(&mut hasher);
        q.text.hash(&mut hasher);
    }
    hasher.finish()
}

/// 文件名安全化：去掉路径分隔符和控制字符，避免书名里的 `/` 造成路径穿越。
fn sanitize(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '　',
            c if c.is_control() => ' ',
            c => c,
        })
        .collect();
    let cleaned = cleaned.trim();
    if cleaned.is_empty() {
        "未命名".to_string()
    } else {
        cleaned.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::quotes::Quote;

    fn quote(id: &str, text: &str, book: &str) -> Quote {
        Quote {
            id: id.to_string(),
            text: text.to_string(),
            source: format!("《{book}》"),
            book: book.to_string(),
            chapter: "第一章".to_string(),
        }
    }

    fn temp_corpus(tag: &str) -> (Corpus, PathBuf) {
        let dir = std::env::temp_dir().join(format!("shiyi-corpus-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let corpus = Corpus {
            dir: dir.clone(),
            inner: Mutex::new(Library::default()),
        };
        (corpus, dir)
    }

    #[test]
    fn builtin_books_are_migrated_to_disk() {
        let (corpus, dir) = temp_corpus("builtin");
        corpus.load();
        let books = corpus.books();
        assert_eq!(books.len(), 5, "五本内置书都应迁移");
        assert!(books.iter().all(|b| b.builtin), "都应标记为内置");
        // 磁盘上确实生成了文件
        let files: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .filter(|e| e.path().extension().and_then(|x| x.to_str()) == Some("json"))
            .collect();
        assert_eq!(files.len(), 5, "每本书一个独立文件");
    }

    #[test]
    fn import_adds_book_and_persists_it() {
        let (corpus, dir) = temp_corpus("import");
        corpus.load();
        let res = corpus.import(
            "新增书",
            vec![quote("new-1", "新增的一句", "新增书")],
        );
        assert!(res.ok, "导入应成功: {}", res.message);
        assert!(corpus.books().iter().any(|b| b.name == "新增书"));
        assert!(dir.join("新增书.json").exists(), "应有独立文件");
        // 重新加载后仍在
        corpus.load();
        assert!(corpus.books().iter().any(|b| b.name == "新增书"));
    }

    #[test]
    fn duplicate_book_name_is_rejected() {
        let (corpus, _) = temp_corpus("dupname");
        corpus.load();
        let res = corpus.import("道德经", vec![quote("x-1", "另一句", "道德经")]);
        assert!(!res.ok, "同名应被拒绝");
        assert!(res.message.contains("已在书库"), "提示要说明原因: {}", res.message);
    }

    #[test]
    fn duplicate_content_under_new_name_is_rejected() {
        let (corpus, _) = temp_corpus("dupcontent");
        corpus.load();
        // 模拟「导出一本书，改个名字再导回来」——内容一模一样
        let exported = corpus.export("道德经").expect("导出应成功");
        let mut quotes: Vec<Quote> = serde_json::from_str(&exported).expect("应可解析");
        for q in &mut quotes {
            q.book = "换了名字".to_string();
            q.source = "《换了名字》".to_string();
        }
        let res = corpus.import("换了名字", quotes);
        assert!(!res.ok, "内容重复应被拒绝");
        assert!(
            res.message.contains("完全相同"),
            "提示要说明内容重复: {}",
            res.message
        );
    }

    #[test]
    fn import_validates_fields() {
        let (corpus, _) = temp_corpus("invalid");
        corpus.load();
        assert!(!corpus.import("空书", vec![]).ok, "空书应被拒绝");

        let dup = vec![quote("a-1", "一", "甲"), quote("a-1", "二", "甲")];
        assert!(!corpus.import("甲", dup).ok, "id 重复应被拒绝");

        let no_text = vec![quote("b-1", "   ", "乙")];
        assert!(!corpus.import("乙", no_text).ok, "空正文应被拒绝");
    }

    #[test]
    fn remove_deletes_book_and_its_file() {
        let (corpus, dir) = temp_corpus("remove");
        corpus.load();
        corpus.import("待删书", vec![quote("rm-1", "一句", "待删书")]);
        let res = corpus.remove("待删书");
        assert!(res.ok, "移除应成功: {}", res.message);
        assert!(!corpus.books().iter().any(|b| b.name == "待删书"));
        assert!(!dir.join("待删书.json").exists(), "文件应被删除");
    }

    #[test]
    fn removing_book_clears_its_favorites() {
        let (corpus, _) = temp_corpus("rmfav");
        corpus.load();
        corpus.import("收藏测试", vec![quote("cf-1", "一句", "收藏测试")]);
        let store = crate::store::DataStore::global();
        store.toggle_favorite("cf-1");
        assert!(store.is_favorite("cf-1"));

        corpus.remove("收藏测试");
        assert!(!store.is_favorite("cf-1"), "移除书籍应清掉它的收藏");
    }

    #[test]
    fn export_round_trips_through_import() {
        let (corpus, _) = temp_corpus("export");
        corpus.load();
        let json = corpus.export("道德经").expect("导出应成功");
        let quotes: Vec<Quote> = serde_json::from_str(&json).expect("导出的 JSON 应可解析");
        assert_eq!(quotes.len(), 79);
        assert!(quotes.iter().all(|q| q.book == "道德经"));
    }

    #[test]
    fn removing_builtin_book_does_not_resurrect_it() {
        let (corpus, dir) = temp_corpus("rmbuiltin");
        corpus.load();
        assert!(corpus.books().iter().any(|b| b.name == "论语"));

        let res = corpus.remove("论语");
        assert!(res.ok, "移除应成功: {}", res.message);

        // 重新加载（模拟重启）后，论语不应该回来
        corpus.load();
        assert!(
            !corpus.books().iter().any(|b| b.name == "论语"),
            "移除的内置书不该在重启后复活"
        );
        assert!(!dir.join("论语.json").exists(), "文件应被删除");
    }

    #[test]
    fn manifest_is_not_treated_as_a_book() {
        let (corpus, dir) = temp_corpus("manifest");
        corpus.load();
        corpus.remove("论语");
        corpus.load();
        // 清单文件存在，但不该被当成一本书
        assert!(dir.join(MANIFEST_FILE).exists(), "清单应写入磁盘");
        assert!(
            !corpus.books().iter().any(|b| b.name == "library"),
            "清单文件不该出现在书库里"
        );
    }

    #[test]
    fn file_name_is_sanitized() {
        let (corpus, dir) = temp_corpus("sanitize");
        corpus.load();
        corpus.import("a/b", vec![quote("s-1", "一句", "a/b")]);
        // 斜杠被替换掉，文件仍在书库目录里，没有跑到子目录
        let files: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .filter(|e| e.path().is_file())
            .collect();
        assert!(
            files.iter().all(|f| f.path().parent() == Some(dir.as_path())),
            "不应产生子目录"
        );
    }

    /// 交付的扩展语料（`语料/` 目录）能整批导入 app。
    ///
    /// 这批文件是外部产出的，不走 `include_str!` 编译期内嵌，
    /// 所以只能靠导入这条路径进来——这里模拟一遍导入前的校验，
    /// 确保每本书都过得了 app 自己的规则。
    #[test]
    fn delivered_corpus_files_pass_import_checks() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("语料");
        if !dir.exists() {
            // 没生成语料目录时跳过（比如只编译 crate 的场景）
            return;
        }

        let mut checked = 0;
        let mut entries: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .filter(|e| {
                e.path().extension().and_then(|x| x.to_str()) == Some("json")
                    // app 自己的清单叫 library.json，交付语料的清单叫 manifest.json，
                    // 两者都不是书，不该按语录解析
                    && !matches!(
                        e.path().file_name().and_then(|x| x.to_str()),
                        Some(MANIFEST_FILE) | Some("manifest.json")
                    )
            })
            .collect();
        entries.sort_by_key(|e| e.path());

        for entry in entries {
            let path = entry.path();
            let raw = fs::read_to_string(&path).expect("语料文件应可读");
            let quotes: Vec<Quote> =
                serde_json::from_str(&raw).unwrap_or_else(|e| panic!("{} 解析失败: {e}", path.display()));

            assert!(!quotes.is_empty(), "{} 是空的", path.display());

            let mut ids = std::collections::HashSet::new();
            for q in &quotes {
                assert!(!q.id.trim().is_empty(), "{} 有语录缺 id", path.display());
                assert!(!q.text.trim().is_empty(), "{} 的 {} 缺正文", path.display(), q.id);
                assert!(!q.book.trim().is_empty(), "{} 的 {} 缺书名", path.display(), q.id);
                assert!(!q.chapter.trim().is_empty(), "{} 的 {} 缺篇章", path.display(), q.id);
                assert_eq!(
                    q.source,
                    format!("《{}》", q.book),
                    "{} 的 {} 出处与书名不一致",
                    path.display(),
                    q.id
                );
                assert!(
                    ids.insert(q.id.clone()),
                    "{} 的 id 重复: {}",
                    path.display(),
                    q.id
                );
            }
            checked += 1;
        }

        assert!(checked > 0, "语料目录里应该有书");
    }
}
