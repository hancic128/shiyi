use serde::{Deserialize, Serialize};

/// 一条语录。语料由运行时的书库加载（见 `crate::corpus`），每本书一个独立文件。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Quote {
    pub id: String,
    pub text: String,
    pub source: String,
    pub book: String,
    pub chapter: String,
}

impl Quote {
    /// 展示用出处：有篇章时为「《书名》· 篇章」。
    pub fn display_source(&self) -> String {
        format!("{} · {}", self.source, self.chapter)
    }
}

/// 加载全部语录（来自运行时的书库）。
///
/// 语料现在放在运行时的书库里（`crate::corpus`），每本书一个独立文件，
/// 用户可以在设置里导入 / 导出 / 移除。
pub fn all() -> Vec<Quote> {
    crate::corpus::Corpus::global().all()
}

/// 内置语料。编译期嵌入，首次启动时由 `corpus` 迁移到书库目录。
///
/// 用户可以在书库里增删书籍，所以 `all()` 的条数会变；校验内置语料的测试
/// 要用这个函数，而不是 `all()`。
#[cfg(test)]
pub fn builtin() -> Vec<Quote> {
    let mut out = Vec::with_capacity(265);
    for (_, raw) in crate::corpus::BUILTIN_RAW {
        match serde_json::from_str::<Vec<Quote>>(raw) {
            Ok(items) => out.extend(items),
            Err(err) => panic!("语料解析失败: {err}"),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    /// 内置语料不会被用户改动，所以校验它。
    /// （`all()` 来自可增删的书库，条数会随用户操作变化。）
    fn builtin() -> Vec<Quote> {
        super::builtin()
    }

    #[test]
    fn corpus_loads_with_expected_shape() {
        let quotes = builtin();
        assert_eq!(quotes.len(), 265, "语料总数应为 265");
    }

    #[test]
    fn ids_are_unique() {
        let quotes = builtin();
        let ids: HashSet<_> = quotes.iter().map(|q| &q.id).collect();
        assert_eq!(ids.len(), quotes.len(), "id 存在重复");
    }

    #[test]
    fn texts_are_unique() {
        let quotes = builtin();
        let texts: HashSet<_> = quotes.iter().map(|q| &q.text).collect();
        assert_eq!(texts.len(), quotes.len(), "正文存在重复");
    }

    #[test]
    fn book_counts_match_spec() {
        let quotes = builtin();
        let count = |book: &str| quotes.iter().filter(|q| q.book == book).count();
        assert_eq!(count("道德经"), 79);
        assert_eq!(count("论语"), 78);
        assert_eq!(count("庄子"), 33);
        assert_eq!(count("孟子"), 40);
        assert_eq!(count("传习录"), 35);
    }

    #[test]
    fn every_field_is_populated() {
        for q in builtin() {
            assert!(!q.id.is_empty(), "id 为空");
            assert!(!q.text.is_empty(), "{} 正文为空", q.id);
            assert!(!q.source.is_empty(), "{} 出处为空", q.id);
            assert!(!q.book.is_empty(), "{} 书名为空", q.id);
            assert!(!q.chapter.is_empty(), "{} 篇章为空", q.id);
        }
    }

    #[test]
    fn display_source_includes_chapter() {
        let q = builtin().remove(0);
        assert!(q.display_source().contains(&q.chapter));
    }

    #[test]
    fn library_contains_builtin_books_on_first_load() {
        // 书库初始化后内置的五本书都在
        let books = crate::corpus::Corpus::global().books();
        for name in ["道德经", "论语", "庄子", "孟子", "传习录"] {
            assert!(
                books.iter().any(|b| b.name == name),
                "书库里应有《{name}》"
            );
        }
    }
}
