<script setup lang="ts">
/**
 * 书库管理面板。
 *
 * 托盘菜单里「书库」和「设置」同级，点开就是这个独立窗口。
 * 列出已安装的书籍，每本书可以：
 * - 导出：写成 JSON 文件
 * - 移除：从书库里删掉（连带清掉它的收藏）
 *
 * 底部可以导入新书。后端会挡住重复导入（同名 / 内容完全一样），
 * 这里把后端返回的中文提示原样显示出来。
 */
import { onMounted, onUnmounted, ref } from 'vue'
import { listen } from '@tauri-apps/api/event'
import { open, save } from '@tauri-apps/plugin-dialog'
import { writeTextFile } from '@tauri-apps/plugin-fs'
import type { BookInfo } from '@/types'
import { api } from '@/api'

const books = ref<BookInfo[]>([])
/** 操作结果提示。`kind` 决定颜色。 */
const message = ref<{ text: string; kind: 'ok' | 'err' } | null>(null)
/** 正在操作的书籍名，用来禁用该行的按钮 */
const busy = ref<string | null>(null)
/** 正在导入 */
const importing = ref(false)

let unlisten: (() => void) | null = null

/** 显示一条提示。3 秒后自动消失，错误提示留久一点。 */
function notify(text: string, kind: 'ok' | 'err' = 'ok') {
  message.value = { text, kind }
  window.clearTimeout(notify.timer)
  notify.timer = window.setTimeout(() => {
    message.value = null
  }, kind === 'err' ? 5000 : 3000)
}
notify.timer = 0

async function reload() {
  books.value = await api.books()
}

onMounted(async () => {
  await reload()
  // 面板窗口是常驻隐藏窗口（关掉只隐藏、不销毁），onMounted 只在启动时跑一次。
  // 书库内容后来变了要靠事件刷新，否则这里显示的是启动那一刻的快照。
  unlisten = await listen('data-changed', () => {
    void reload()
  })
})

onUnmounted(() => {
  unlisten?.()
})

/** 导入一本书。 */
async function importBook() {
  if (importing.value) return
  importing.value = true
  try {
    const picked = await open({
      multiple: false,
      directory: false,
      title: '选择语料文件',
      filters: [{ name: '语料 JSON', extensions: ['json'] }],
    })
    // 用户取消了
    if (!picked || Array.isArray(picked)) return

    const result = await api.importBook(picked)
    notify(result.message, result.ok ? 'ok' : 'err')
    if (result.ok) await reload()
  } catch (err) {
    notify(`导入失败：${String(err)}`, 'err')
  } finally {
    importing.value = false
  }
}

/** 导出一本书。 */
async function exportBook(book: BookInfo) {
  if (busy.value) return
  busy.value = book.name
  try {
    const result = await api.exportBook(book.name)
    if (!result.ok || !result.content) {
      notify(result.message, 'err')
      return
    }

    const target = await save({
      title: '导出语料',
      defaultPath: `${book.name}.json`,
      filters: [{ name: '语料 JSON', extensions: ['json'] }],
    })
    // 用户取消了保存位置
    if (!target) {
      notify('已取消导出', 'ok')
      return
    }

    await writeTextFile(target, result.content)
    notify(`已导出《${book.name}》`, 'ok')
  } catch (err) {
    notify(`导出失败：${String(err)}`, 'err')
  } finally {
    busy.value = null
  }
}

/** 移除一本书。 */
async function removeBook(book: BookInfo) {
  if (busy.value) return
  busy.value = book.name
  try {
    const result = await api.removeBook(book.name)
    notify(result.message, result.ok ? 'ok' : 'err')
    if (result.ok) await reload()
  } catch (err) {
    notify(`移除失败：${String(err)}`, 'err')
  } finally {
    busy.value = null
  }
}
</script>

<template>
  <div class="panel">
    <header class="head">
      <h1>书库</h1>
      <span class="count">{{ books.length }} 本</span>
    </header>

    <!-- 操作提示 -->
    <Transition name="toast">
      <p v-if="message" class="msg" :class="message.kind">{{ message.text }}</p>
    </Transition>

    <div class="body">
      <p v-if="books.length === 0" class="empty">书库是空的，先导入一本书吧</p>
      <ul v-else class="list">
        <li v-for="b in books" :key="b.name">
          <div class="info">
            <span class="name">《{{ b.name }}》</span>
            <span class="count">{{ b.count }} 条</span>
            <span v-if="b.builtin" class="tag">内置</span>
          </div>
          <div class="ops">
            <button
              class="op"
              :disabled="busy !== null"
              @click="exportBook(b)"
            >
              导出
            </button>
            <button
              class="op danger"
              :disabled="busy !== null"
              @click="removeBook(b)"
            >
              移除
            </button>
          </div>
        </li>
      </ul>
    </div>

    <footer class="foot">
      <button class="import" :disabled="importing" @click="importBook">
        {{ importing ? '导入中…' : '导入书籍…' }}
      </button>
    </footer>
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
  border-bottom: 1px solid var(--border);
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

/* 操作提示 */
.msg {
  margin: 0;
  padding: 8px 18px;
  font-size: 12px;
  border-bottom: 1px solid var(--border);
}

.msg.ok {
  color: var(--accent);
  background: var(--accent-soft);
}

.msg.err {
  color: #c0392b;
  background: rgba(192, 57, 43, 0.1);
}

.toast-enter-active,
.toast-leave-active {
  transition: opacity 0.18s ease;
}

.toast-enter-from,
.toast-leave-to {
  opacity: 0;
}

.body {
  flex: 1;
  overflow-y: auto;
  padding: 4px 0;
}

.empty {
  text-align: center;
  color: var(--fg-muted);
  font-size: 13px;
  padding: 40px 16px;
  margin: 0;
}

.list {
  list-style: none;
  margin: 0;
  padding: 0;
}

.list li {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  padding: 10px 18px;
}

.list li:hover {
  background: var(--accent-soft);
}

.info {
  display: flex;
  align-items: baseline;
  gap: 8px;
  min-width: 0;
}

.name {
  font-size: 13px;
  font-family: var(--font-serif);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.count {
  font-size: 11px;
  color: var(--fg-muted);
  white-space: nowrap;
}

.tag {
  font-size: 10px;
  color: var(--accent);
  background: var(--accent-soft);
  border-radius: 3px;
  padding: 1px 5px;
  white-space: nowrap;
}

.ops {
  display: flex;
  gap: 6px;
  flex-shrink: 0;
}

.op {
  background: none;
  border: 1px solid var(--border);
  border-radius: 5px;
  color: var(--fg-muted);
  font-size: 11px;
  padding: 3px 10px;
  transition: all 0.15s ease;
}

.op:hover:not(:disabled) {
  color: var(--accent);
  border-color: var(--accent);
}

.op.danger:hover:not(:disabled) {
  color: #c0392b;
  border-color: #c0392b;
}

.op:disabled {
  opacity: 0.45;
  cursor: not-allowed;
}

.foot {
  padding: 10px 18px 14px;
  border-top: 1px solid var(--border);
}

.import {
  width: 100%;
  background: var(--accent);
  color: #fff;
  border: none;
  border-radius: var(--radius);
  padding: 8px;
  font-size: 13px;
  transition: opacity 0.15s ease;
}

.import:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}
</style>
