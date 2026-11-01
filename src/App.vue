<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref } from 'vue'
import { listen } from '@tauri-apps/api/event'
import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow'
import type { Quote, Settings } from '@/types'
import { api } from '@/api'
import { applyAppearance } from '@/appearance'
import QuoteCard from '@/components/QuoteCard.vue'
import FavoritesPanel from '@/components/FavoritesPanel.vue'
import HistoryPanel from '@/components/HistoryPanel.vue'
import SettingsPanel from '@/components/SettingsPanel.vue'
import BookLibraryPanel from '@/components/BookLibraryPanel.vue'

/** 当前窗口的 label，决定渲染哪个面板。 */
const label = getCurrentWebviewWindow().label

const settings = ref<Settings | null>(null)
const quote = ref<Quote | null>(null)
const pinned = ref(false)
const cardEl = ref<HTMLElement | null>(null)

/** 卡片自动消失的定时器句柄。 */
let dismissTimer: ReturnType<typeof setTimeout> | null = null

const isCard = computed(() => label === 'card')

/** 载入设置。 */
async function loadSettings() {
  settings.value = await api.settings()
}

/**
 * 语录长短不一，窗口高度需要跟着内容走。
 * 测量卡片元素的实际高度后调整原生窗口尺寸。
 */
async function fitCard() {
  await nextTick()
  const el = cardEl.value
  if (!el) return
  const height = el.getBoundingClientRect().height
  if (height > 0) {
    await api.resizeCard(height)
  }
}

/** 后端推送新语录。 */
async function onShowQuote(event: { payload: Quote }) {
  quote.value = event.payload
  // 重新读一次设置：用户在设置窗口改了主题 / 停留时长，这里要跟上
  await loadSettings()
  await fitCard()
  scheduleDismiss()
}

/** 到期自动隐藏卡片（固定状态下不隐藏）。 */
function scheduleDismiss() {
  if (dismissTimer) {
    clearTimeout(dismissTimer)
    dismissTimer = null
  }
  const seconds = settings.value?.autoDismissSeconds ?? 8
  if (seconds <= 0) return
  dismissTimer = setTimeout(() => {
    if (!pinned.value) hide()
  }, seconds * 1000)
}

async function hide() {
  await api.hideCard()
}

async function onClosed() {
  await api.hideCard()
}

/** 固定 / 取消固定。固定后卡片不再自动消失。 */
function onPinned(value: boolean) {
  pinned.value = value
  if (!value) scheduleDismiss()
}

/** 从收藏夹 / 历史里选一条。回看历史不算一次新的弹出，不重复记历史。 */
async function onSelect(chosen: Quote) {
  await api.showCard(chosen, false)
}

let unlisten: (() => void) | null = null
let unlistenAppearance: (() => void) | null = null

onMounted(async () => {
  await loadSettings()
  applyAppearance(settings.value?.appearanceMode ?? 'auto')

  // 外观在其他窗口切换时，本窗口也要跟着变
  unlistenAppearance = await listen<string>('appearance-changed', (event) => {
    applyAppearance(event.payload)
  })

  if (isCard.value) {
    // 首次启动时先取一条，卡片窗口一出现就有内容
    if (!quote.value) {
      quote.value = await api.randomQuote()
      if (quote.value) {
        await fitCard()
        scheduleDismiss()
      }
    }
    unlisten = await listen<Quote>('show-quote', (event) => {
      void onShowQuote(event)
    })
  }
})

onUnmounted(() => {
  if (dismissTimer) clearTimeout(dismissTimer)
  if (unlisten) unlisten()
  if (unlistenAppearance) unlistenAppearance()
})
</script>

<template>
  <div v-if="isCard" ref="cardEl" class="card-wrap">
    <QuoteCard
      :quote="quote"
      :settings="settings ?? { theme: 'xuanzhi', autoDismissSeconds: 8 }"
      :pinned="pinned"
      @update:pinned="onPinned"
      @closed="onClosed"
    />
  </div>

  <FavoritesPanel v-else-if="label === 'favorites'" @select="onSelect" />

  <HistoryPanel v-else-if="label === 'history'" @select="onSelect" />

  <SettingsPanel v-else-if="label === 'settings'" />

  <BookLibraryPanel v-else-if="label === 'library'" />

  <div v-else class="fallback">拾遗</div>
</template>

<style scoped>
/* 卡片窗口是透明窗口：给容器留出圆角与投影的余量，
   否则阴影会被窗口边界裁掉。 */
.card-wrap {
  padding: 12px;
}

.fallback {
  display: flex;
  align-items: center;
  justify-content: center;
  height: 100vh;
  font-family: var(--font-serif);
  color: var(--fg-muted);
}
</style>
