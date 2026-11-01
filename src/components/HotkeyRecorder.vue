<script setup lang="ts">
import { onBeforeUnmount, ref, watch } from 'vue'

const props = defineProps<{ modelValue: string }>()
const emit = defineEmits<{ (e: 'update:modelValue', value: string): void }>()

const recording = ref(false)

/** 修饰键 → Tauri 快捷键语法里的名字。 */
const MODIFIERS: { key: 'metaKey' | 'ctrlKey' | 'altKey' | 'shiftKey'; name: string }[] = [
  { key: 'metaKey', name: 'CmdOrCtrl' },
  { key: 'ctrlKey', name: 'Ctrl' },
  { key: 'altKey', name: 'Alt' },
  { key: 'shiftKey', name: 'Shift' },
]

/** 只接受单个字母或数字，避免录到功能键这类没法映射的键。 */
const VALID_KEY = /^[a-z0-9]$/i

/** 把 KeyboardEvent 翻译成 "CmdOrCtrl+Shift+L" 这样的字符串。 */
function toShortcut(e: KeyboardEvent): string | null {
  const parts: string[] = []
  for (const { key, name } of MODIFIERS) {
    if (e[key]) parts.push(name)
  }
  // 组合键必须至少带一个修饰键，否则会抢占普通按键
  if (parts.length === 0) return null

  if (!VALID_KEY.test(e.key)) return null
  return `${parts.join('+')}+${e.key.toUpperCase()}`
}

/** 录制时全局监听按键。设置窗口失焦也要能收到，所以挂在 window 上。 */
function onKeyDown(e: KeyboardEvent) {
  if (!recording.value) return
  e.preventDefault()
  e.stopPropagation()
  if (e.key === 'Escape') {
    recording.value = false
    return
  }
  const shortcut = toShortcut(e)
  if (shortcut) {
    emit('update:modelValue', shortcut)
    recording.value = false
  }
}

function start() {
  recording.value = true
}

function cancel() {
  recording.value = false
}

watch(recording, (on) => {
  if (on) {
    window.addEventListener('keydown', onKeyDown, true)
  } else {
    window.removeEventListener('keydown', onKeyDown, true)
  }
})

onBeforeUnmount(() => {
  window.removeEventListener('keydown', onKeyDown, true)
})
</script>

<template>
  <div class="recorder">
    <span class="value" :class="{ recording }">
      {{ recording ? '按下组合键…' : props.modelValue }}
    </span>
    <button v-if="recording" class="btn ghost" @click="cancel">取消</button>
    <button v-else class="btn" @click="start">录制</button>
  </div>
</template>

<style scoped>
.recorder {
  display: flex;
  align-items: center;
  gap: 8px;
  justify-content: flex-end;
}

.value {
  font-family: var(--font-sans);
  font-size: 13px;
  font-weight: 500;
  padding: 3px 10px;
  border-radius: 6px;
  background: rgba(128, 128, 128, 0.1);
  min-width: 120px;
  text-align: center;
}

.value.recording {
  color: var(--accent);
  background: var(--accent-soft);
}

.btn {
  border: 1px solid var(--border);
  background: var(--bg-elevated);
  color: var(--fg);
  border-radius: 5px;
  font-size: 12px;
  padding: 3px 10px;
  transition: all 0.15s ease;
}

.btn:hover {
  border-color: var(--accent);
  color: var(--accent);
}

.btn.ghost {
  background: none;
}
</style>
