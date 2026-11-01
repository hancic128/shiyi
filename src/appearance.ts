/**
 * 界面外观：跟随系统 / 亮色 / 暗色。
 *
 * 原生窗口的标题栏配色由后端 `set_appearance` 切换；网页内部的 CSS 变量
 * 由这里改写——两边的 `prefers-color-scheme` 媒体查询在 Tauri 里会跟随
 * 系统走，用户手动选「亮色」时系统仍是深色，所以必须用 data 属性强制覆盖。
 */
export type AppearanceMode = 'auto' | 'light' | 'dark'

/** 把外观模式写到 <html> 的 data-appearance 上。auto 时移除属性，跟随系统。 */
export function applyAppearance(mode: AppearanceMode | string) {
  const root = document.documentElement
  if (mode === 'light' || mode === 'dark') {
    root.setAttribute('data-appearance', mode)
  } else {
    root.removeAttribute('data-appearance')
  }
}
