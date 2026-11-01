/** 语录条目，与 Rust 侧 `Quote` 结构一一对应。 */
export interface Quote {
  id: string
  text: string
  source: string
  book: string
  chapter: string
}

/** 书库里的一本书，与 Rust 侧 `BookInfo` 结构一一对应。 */
export interface BookInfo {
  /** 书名，如「道德经」。 */
  name: string
  /** 语录条数。 */
  count: number
  /** 语录 id 前缀。 */
  prefix: string
  /** 是否是随应用分发的内置书籍。 */
  builtin: boolean
}

/** 导入 / 导出 / 移除的结果，与 Rust 侧 `BookOpResult` 一一对应。 */
export interface BookOpResult {
  /** 是否成功。 */
  ok: boolean
  /** 给用户看的中文提示（成功或失败原因）。 */
  message: string
}

/** 导出结果，与 Rust 侧 `ExportResult` 一一对应。 */
export interface ExportResult extends BookOpResult {
  /** 导出的 JSON 文本，由前端写盘。 */
  content: string | null
}

/** 历史记录条目，与 Rust 侧 `HistoryEntry` 结构一一对应。 */
export interface HistoryEntry {
  id: string
  quoteId: string
  timestamp: number
  text: string
  source: string
}

/** 用户设置，与 Rust 侧 `Settings` 结构一一对应。 */
export interface Settings {
  startHour: number
  endHour: number
  popupsPerDay: number
  skipWeekend: boolean
  autoDismissSeconds: number
  theme: string
  appearanceMode: string
  hotkey: string
  launchAtLogin: boolean
}

export type ThemeName = 'xuanzhi' | 'ink' | 'bamboo' | 'night' | 'jade' | 'random'

export interface ThemePalette {
  background: string
  stroke: string
  text: string
  source: string
  sealBg: string
  sealText: string
  buttonNormal: string
  buttonActive: string
  hoverBg: string
  shadow: string
}
