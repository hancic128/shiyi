<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue'
import { listen } from '@tauri-apps/api/event'
import type { Quote } from '@/types'
import { api } from '@/api'

const emit = defineEmits<{ (e: 'select', quote: Quote): void }>()

const quotes = ref<Quote[]>([])
const favorites = ref<Set<string>>(new Set())
const filterBook = ref('全部')

let unlisten: (() => void) | null = null

/** 重新拉取收藏列表。语料是编译期内嵌的静态数据，只需在挂载时取一次。 */
async function reload() {
  favorites.value = new Set(await api.favorites())
}

onMounted(async () => {
  quotes.value = await api.quotes()
  await reload()
  // 面板窗口是常驻隐藏窗口（关掉只隐藏、不销毁），onMounted 只在启动时跑一次。
  // 收藏状态后来变了要靠事件刷新，否则这里显示的是启动那一刻的快照。
  unlisten = await listen('data-changed', () => {
    void reload()
  })
})

onUnmounted(() => {
  unlisten?.()
})

const books = computed(() => {
  const set = new Set(
    quotes.value.filter((q) => favorites.value.has(q.id)).map((q) => q.book),
  )
  return ['全部', ...[...set].sort()]
})

const filtered = computed(() => {
  const favs = quotes.value.filter((q) => favorites.value.has(q.id))
  return filterBook.value === '全部'
    ? favs
    : favs.filter((q) => q.book === filterBook.value)
})

async function removeFavorite(id: string) {
  await api.toggleFavorite(id)
  favorites.value = new Set(await api.favorites())
}
</script>

<template>
  <div class="panel">
    <header class="head">
      <h1>收藏夹</h1>
      <span class="count">{{ filtered.length }} 条</span>
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
      <p v-if="filtered.length === 0" class="empty">暂无收藏</p>
      <ul v-else class="list">
        <li v-for="q in filtered" :key="q.id">
          <div class="row" @click="emit('select', q)">
            <p class="text">{{ q.text }}</p>
            <div class="meta">
              <span class="book">{{ q.book }}</span>
              <span class="src">{{ q.source }} · {{ q.chapter }}</span>
              <button class="rm" title="取消收藏" @click.stop="removeFavorite(q.id)">✕</button>
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

.count {
  font-size: 12px;
  color: var(--fg-muted);
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

.rm {
  background: none;
  border: none;
  color: var(--fg-muted);
  font-size: 11px;
  padding: 2px 4px;
  opacity: 0;
  transition: opacity 0.15s ease;
}

.row:hover .rm {
  opacity: 1;
}

.rm:hover {
  color: var(--accent);
}
</style>
