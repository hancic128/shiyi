<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue'
import { listen } from '@tauri-apps/api/event'
import type { HistoryEntry, Quote } from '@/types'
import { api } from '@/api'

const emit = defineEmits<{ (e: 'select', quote: Quote): void }>()

const history = ref<HistoryEntry[]>([])
const quotes = ref<Quote[]>([])
const filterBook = ref('全部')

let unlisten: (() => void) | null = null

/** 重新拉取历史。语料是编译期内嵌的静态数据，只需在挂载时取一次。 */
async function reload() {
  history.value = await api.history()
}

onMounted(async () => {
  quotes.value = await api.quotes()
  await reload()
  // 面板窗口是常驻隐藏窗口（关掉只隐藏、不销毁），所以 onMounted 只在启动时
  // 跑一次。数据后来变了（弹出新语录、收藏夹里取消收藏）必须靠事件刷新，
  // 否则这里会一直显示启动那一刻的旧内容。
  unlisten = await listen('data-changed', () => {
    void reload()
  })
})

onUnmounted(() => {
  unlisten?.()
})

function bookOf(entry: HistoryEntry): string {
  return quotes.value.find((q) => q.id === entry.quoteId)?.book ?? '其他'
}

const books = computed(() => {
  const set = new Set(history.value.map(bookOf))
  return ['全部', ...[...set].sort()]
})

const filtered = computed(() =>
  filterBook.value === '全部'
    ? history.value
    : history.value.filter((e) => bookOf(e) === filterBook.value),
)

function formatTime(ts: number): string {
  return new Date(ts).toLocaleString('zh-CN', {
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
  })
}

async function select(entry: HistoryEntry) {
  const q = quotes.value.find((x) => x.id === entry.quoteId)
  if (q) emit('select', q)
}

async function clear() {
  await api.clearHistory()
  history.value = []
}
</script>

<template>
  <div class="panel">
    <header class="head">
      <h1>历史记录</h1>
      <div class="right">
        <span class="count">{{ filtered.length }} 条</span>
        <button v-if="history.length" class="clear" @click="clear">清空</button>
      </div>
    </header>

    <nav class="filters">
      <button
        v-for="b in books"
        :key="b"
        class="chip"
        :class="{ on: filterBook === b }"
        @click="filterBook = b"
      >
        {{ b }}
      </button>
    </nav>

    <div class="body">
      <p v-if="filtered.length === 0" class="empty">暂无记录</p>
      <ul v-else class="list">
        <li v-for="e in filtered" :key="e.id">
          <div class="row" @click="select(e)">
            <p class="text">{{ e.text }}</p>
            <div class="meta">
              <span class="book">{{ bookOf(e) }}</span>
              <span class="src">{{ e.source }}</span>
              <span class="time">{{ formatTime(e.timestamp) }}</span>
            </div>
          </div>
        </li>
      </ul>
    </div>
  </div>
</template>

<style scoped>
.panel {
  display: flex;
  flex-direction: column;
  height: 100vh;
  background: var(--bg);
}

.head {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  padding: 16px 18px 10px;
}

.head h1 {
  margin: 0;
  font-family: var(--font-serif);
  font-size: 17px;
  font-weight: 600;
}

.right {
  display: flex;
  align-items: baseline;
  gap: 10px;
}

.count {
  font-size: 12px;
  color: var(--fg-muted);
}

.clear {
  background: none;
  border: 1px solid var(--border);
  border-radius: 5px;
  color: var(--fg-muted);
  font-size: 11px;
  padding: 2px 8px;
}

.clear:hover {
  color: var(--accent);
  border-color: var(--accent);
}

.filters {
  display: flex;
  gap: 6px;
  padding: 0 18px 12px;
  flex-wrap: wrap;
}

.chip {
  border: 1px solid var(--border);
  background: var(--bg-elevated);
  color: var(--fg);
  border-radius: 999px;
  padding: 3px 11px;
  font-size: 12px;
  transition: all 0.15s ease;
}

.chip:hover {
  border-color: var(--accent);
}

.chip.on {
  background: var(--accent);
  border-color: var(--accent);
  color: #fff;
}

.body {
  flex: 1;
  overflow-y: auto;
  padding: 0 12px 14px;
}

.empty {
  text-align: center;
  color: var(--fg-muted);
  font-size: 13px;
  padding: 40px 0;
}

.list {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.row {
  border-radius: var(--radius);
  padding: 10px 12px;
  cursor: pointer;
  transition: background 0.15s ease;
}

.row:hover {
  background: var(--accent-soft);
}

.text {
  margin: 0 0 6px;
  font-family: var(--font-serif);
  font-size: 14px;
  line-height: 1.65;
  display: -webkit-box;
  -webkit-line-clamp: 3;
  -webkit-box-orient: vertical;
  overflow: hidden;
}

.meta {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 11px;
  color: var(--fg-muted);
}

.book {
  background: var(--accent-soft);
  color: var(--accent);
  border-radius: 4px;
  padding: 1px 6px;
}

.src {
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.time {
  white-space: nowrap;
}
</style>
