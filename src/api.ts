import { invoke } from '@tauri-apps/api/core'
import type { BookInfo, BookOpResult, ExportResult, HistoryEntry, Quote, Settings } from '@/types'

/** 调用后端命令的薄封装。所有后端交互集中在这里，便于统一错误处理。 */
export const api = {
  quotes: () => invoke<Quote[]>('get_quotes'),
  randomQuote: () => invoke<Quote | null>('get_random_quote'),
  quoteById: (id: string) => invoke<Quote | null>('get_quote_by_id', { id }),

  favorites: () => invoke<string[]>('get_favorites'),
  isFavorite: (id: string) => invoke<boolean>('is_favorite', { id }),
  toggleFavorite: (id: string) => invoke<boolean>('toggle_favorite', { id }),

  history: () => invoke<HistoryEntry[]>('get_history'),
  clearHistory: () => invoke<void>('clear_history'),

  settings: () => invoke<Settings>('get_settings'),
  saveSettings: (settings: Settings) => invoke<Settings>('save_settings', { settings }),

  showCard: (quote?: Quote, record = true) =>
    invoke<void>('show_card', { quote: quote ?? null, record }),
  hideCard: () => invoke<void>('hide_card'),
  hideSettings: () => invoke<void>('hide_settings'),
  resizeCard: (height: number) => invoke<void>('resize_card', { height }),
  openWindow: (label: string) => invoke<void>('open_window', { label }),

  nextFireDate: () => invoke<number | null>('get_next_fire_date'),
  setAutostart: (enabled: boolean) => invoke<boolean>('set_autostart', { enabled }),
  isAutostartEnabled: () => invoke<boolean>('is_autostart_enabled'),
  setAppearance: (mode: string) => invoke<void>('set_appearance', { mode }),

  // 书库
  books: () => invoke<BookInfo[]>('get_books'),
  importBook: (path: string) => invoke<BookOpResult>('import_book', { path }),
  exportBook: (name: string) => invoke<ExportResult>('export_book', { name }),
  removeBook: (name: string) => invoke<BookOpResult>('remove_book', { name }),
}
