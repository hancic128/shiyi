import type { ThemeName, ThemePalette } from '@/types'

export interface ThemeDef {
  name: ThemeName
  label: string
  palette: ThemePalette
}

/** 五种卡片主题，配色沿用「宣纸古典 / 水墨 / 竹简 / 夜读 / 青玉」。 */
export const THEMES: ThemeDef[] = [
  {
    name: 'xuanzhi',
    label: '宣纸古典',
    palette: {
      background: '#f4edda',
      stroke: 'rgba(140,107,77,0.30)',
      text: '#2e241a',
      source: '#66523d',
      sealBg: '#9e2924',
      sealText: '#ffffff',
      buttonNormal: '#66523d',
      buttonActive: '#9e2924',
      hoverBg: 'rgba(140,107,77,0.16)',
      shadow: 'rgba(0,0,0,0.18)',
    },
  },
  {
    name: 'ink',
    label: '水墨',
    palette: {
      background: '#f7f7f5',
      stroke: 'rgba(0,0,0,0.14)',
      text: '#141414',
      source: '#4d4d4d',
      sealBg: '#111111',
      sealText: '#ffffff',
      buttonNormal: '#404040',
      buttonActive: '#111111',
      hoverBg: 'rgba(0,0,0,0.07)',
      shadow: 'rgba(0,0,0,0.12)',
    },
  },
  {
    name: 'bamboo',
    label: '竹简',
    palette: {
      background: '#c7b073',
      stroke: 'rgba(77,56,26,0.38)',
      text: '#1f1a0f',
      source: '#47331a',
      sealBg: '#33240f',
      sealText: '#d9bf73',
      buttonNormal: '#47331a',
      buttonActive: '#33240f',
      hoverBg: 'rgba(51,36,15,0.20)',
      shadow: 'rgba(0,0,0,0.22)',
    },
  },
  {
    name: 'night',
    label: '夜读',
    palette: {
      background: '#1a1a21',
      stroke: 'rgba(255,255,255,0.12)',
      text: '#ebebe8',
      source: '#a6a69e',
      sealBg: '#c7944d',
      sealText: '#1a1a21',
      buttonNormal: '#b3b3ae',
      buttonActive: '#c7944d',
      hoverBg: 'rgba(255,255,255,0.09)',
      shadow: 'rgba(0,0,0,0.45)',
    },
  },
  {
    name: 'jade',
    label: '青玉',
    palette: {
      background: '#e0f2ed',
      stroke: 'rgba(32,80,67,0.22)',
      text: '#0f3329',
      source: '#2e6b5c',
      sealBg: '#2e735f',
      sealText: '#ffffff',
      buttonNormal: '#2e6b5c',
      buttonActive: '#1f5948',
      hoverBg: 'rgba(46,115,95,0.16)',
      shadow: 'rgba(0,0,0,0.15)',
    },
  },
]

/** 主题为「随机」时，每次弹出随机取一种具体主题。 */
export function resolveTheme(raw: string): ThemeDef {
  if (raw === 'random') {
    const idx = Math.floor(Math.random() * THEMES.length)
    return THEMES[idx]
  }
  return THEMES.find((t) => t.name === raw) ?? THEMES[0]
}
