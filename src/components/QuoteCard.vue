<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow'
import type { Quote } from '@/types'
import { resolveTheme } from '@/themes'
import { api } from '@/api'

const props = defineProps<{
  quote: Quote | null
  settings: { theme: string; autoDismissSeconds: number }
  pinned: boolean
}>()

const emit = defineEmits<{
  (e: 'update:pinned', value: boolean): void
  (e: 'closed'): void
}>()

const palette = computed(() => resolveTheme(props.settings.theme).palette)
const isFavorite = ref(false)
const copied = ref(false)

/** 卡片整体由系统窗口承载，拖动靠 Tauri 的 startDragging（原生实现，不会有残影）。 */
async function startDrag() {
  await getCurrentWebviewWindow().startDragging()
}

async function syncFavorite() {
  if (!props.quote) {
    isFavorite.value = false
    return
  }
  isFavorite.value = await api.isFavorite(props.quote.id)
}

watch(() => props.quote, syncFavorite, { immediate: true })

async function toggleFavorite() {
  if (!props.quote) return
  isFavorite.value = await api.toggleFavorite(props.quote.id)
}

async function copy() {
  if (!props.quote) return
  const text = `${props.quote.text}\n—— ${props.quote.source} · ${props.quote.chapter}`
  await navigator.clipboard.writeText(text)
  copied.value = true
  setTimeout(() => (copied.value = false), 1200)
}

/** 换一条：交给后端重新选一句并重新弹出。 */
async function next() {
  await api.showCard()
}

function togglePin() {
  emit('update:pinned', !props.pinned)
}
</script>

<template>
  <div
    class="card"
    :style="{
      background: palette.background,
      borderColor: palette.stroke,
      boxShadow: `0 10px 30px ${palette.shadow}`,
    }"
  >
    <header class="head" @mousedown="startDrag">
      <span class="seal" :style="{ background: palette.sealBg, color: palette.sealText }">道</span>
      <button
        v-if="!pinned"
        class="close"
        :style="{ color: palette.source }"
        title="关闭"
        @mousedown.stop
        @click="emit('closed')"
      >
        ✕
      </button>
    </header>

    <p class="text" :style="{ color: palette.text }">{{ quote?.text ?? '' }}</p>

    <p class="source" :style="{ color: palette.source }">
      —— {{ quote?.source }} · {{ quote?.chapter }}
    </p>

    <footer class="actions">
      <button
        class="act"
        :style="{ color: isFavorite ? palette.buttonActive : palette.buttonNormal }"
        title="收藏"
        @mousedown.stop
        @click="toggleFavorite"
      >
        <span class="ico">{{ isFavorite ? '♥' : '♡' }}</span>
        <span class="lbl">收藏</span>
      </button>
      <button
        class="act"
        :style="{ color: copied ? palette.buttonActive : palette.buttonNormal }"
        title="复制"
        @mousedown.stop
        @click="copy"
      >
        <span class="ico">⧉</span>
        <span class="lbl">{{ copied ? '已复制' : '复制' }}</span>
      </button>
      <button
        class="act"
        :style="{ color: palette.buttonNormal }"
        title="换一条"
        @mousedown.stop
        @click="next"
      >
        <span class="ico">↻</span>
        <span class="lbl">换一条</span>
      </button>
      <button
        class="act"
        :style="{ color: pinned ? palette.buttonActive : palette.buttonNormal }"
        title="固定"
        @mousedown.stop
        @click="togglePin"
      >
        <!--
          固定图标用内联 SVG 而不是 emoji：
          emoji 会走 Apple Color Emoji 渲染成彩色，和宣纸 / 水墨这几套配色格格不入。
          SVG 用 currentColor 取按钮本身的颜色，未固定是描边、固定是实心，
          和收藏按钮 ♡ → ♥ 的观感一致。
          图形是一枚钉子（图钉），不是地图定位针。
        -->
        <svg
          v-if="!pinned"
          class="ico"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="1.8"
          stroke-linecap="round"
          stroke-linejoin="round"
        >
          <path d="M9 3h6" />
          <path d="M10 3v5.2c0 .9-.4 1.8-1.1 2.4L7 12h10l-1.9-1.4a3 3 0 0 1-1.1-2.4V3" />
          <path d="M12 12v9" />
        </svg>
        <svg
          v-else
          class="ico"
          viewBox="0 0 24 24"
          fill="currentColor"
          stroke="currentColor"
          stroke-width="1.8"
          stroke-linecap="round"
          stroke-linejoin="round"
        >
          <path
            fill-rule="evenodd"
            d="M9 3h6v.8h-1v5.4c0 .9-.4 1.8-1.1 2.4L11 12.9V21h2v.8H9V21h2v-8.1l-1.9-1.4A3 3 0 0 1 8 9.2V3.8H7V3h2zm2 1.8v4.4c0 .5.2 1 .6 1.4l1.9 1.4H12V4.8H9v.6h2z"
          />
        </svg>
        <span class="lbl">{{ pinned ? '已固定' : '固定' }}</span>
      </button>
    </footer>
  </div>
</template>

<style scoped>
.card {
  width: 100%;
  border: 1px solid;
  border-radius: 10px;
  padding: 20px 22px 16px;
  display: flex;
  flex-direction: column;
  gap: 12px;
  user-select: none;
  overflow: hidden;
}

.head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  cursor: grab;
}

.head:active {
  cursor: grabbing;
}

.seal {
  width: 22px;
  height: 22px;
  border-radius: 50%;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  font-family: var(--font-serif);
  font-size: 13px;
  font-weight: 700;
  line-height: 1;
  user-select: none;
}

.close {
  background: none;
  border: none;
  font-size: 12px;
  line-height: 1;
  padding: 4px;
  opacity: 0.55;
  transition: opacity 0.15s ease;
}

.close:hover {
  opacity: 1;
}

.text {
  margin: 0;
  font-family: var(--font-serif);
  font-size: 19px;
  line-height: 1.75;
  letter-spacing: 0.02em;
}

.source {
  margin: 0;
  font-family: var(--font-serif);
  font-size: 12px;
  opacity: 0.8;
  text-align: right;
}

.actions {
  display: flex;
  gap: 6px;
  border-top: 1px solid rgba(128, 128, 128, 0.18);
  padding-top: 10px;
}

.act {
  flex: 1;
  background: none;
  border: none;
  border-radius: 6px;
  padding: 6px 4px;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 3px;
  opacity: 0.85;
  transition:
    opacity 0.15s ease,
    transform 0.15s ease,
    background 0.15s ease;
}

.act:hover {
  opacity: 1;
  transform: translateY(-1px);
}

.act .ico {
  font-size: 14px;
  line-height: 1;
}

/* SVG 图标要和旁边的文字图标一样大 */
.act .ico:is(svg) {
  width: 14px;
  height: 14px;
  display: block;
}

.act .lbl {
  font-size: 10px;
  line-height: 1;
  font-family: var(--font-serif);
  white-space: nowrap;
}
</style>
