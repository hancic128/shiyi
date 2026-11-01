<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import type { Settings } from '@/types'
import { THEMES } from '@/themes'
import { applyAppearance } from '@/appearance'
import { api } from '@/api'
import HotkeyRecorder from '@/components/HotkeyRecorder.vue'

const settings = ref<Settings | null>(null)
const nextFire = ref<number | null>(null)
const autostart = ref(false)
/** 保存后弹出的「已保存」提示 */
const toast = ref(false)

const HOURS = Array.from({ length: 24 }, (_, i) => i)

const themeOptions = computed(() => [...THEMES.map((t) => ({ value: t.name, label: t.label })), {
  value: 'random',
  label: '随机',
}])

const nextFireText = computed(() => {
  if (nextFire.value == null) return '今天不再弹出'
  return new Date(nextFire.value).toLocaleString('zh-CN', {
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit',
  })
})

onMounted(async () => {
  settings.value = await api.settings()
  applyAppearance(settings.value.appearanceMode)
  nextFire.value = await api.nextFireDate()
  autostart.value = await api.isAutostartEnabled()
})

/**
 * 保存设置。
 *
 * 保存成功后弹一个「已保存」的提示，然后自动关掉设置窗口——
 * 设置面板是常驻隐藏窗口，隐藏（而不是销毁）之后还能再打开。
 */
async function save() {
  if (!settings.value) return
  settings.value = await api.saveSettings(settings.value)
  toast.value = true
  // 让提示先渲染出来，再收起窗口；多留一点时间让人看见。
  setTimeout(async () => {
    toast.value = false
    await api.hideSettings()
  }, 900)
}

/** 切换外观：立即生效并落盘，不用等「保存」。 */
async function changeAppearance(mode: string) {
  if (!settings.value) return
  settings.value.appearanceMode = mode
  applyAppearance(mode)
  await api.setAppearance(mode)
  await api.saveSettings(settings.value)
}

/** 时间窗非法时给出提示。 */
const windowValid = computed(() => {
  const s = settings.value
  if (!s) return true
  return s.endHour > s.startHour
})

async function toggleAutostart(e: Event) {
  const target = e.target as HTMLInputElement
  autostart.value = await api.setAutostart(target.checked)
  if (settings.value) {
    settings.value.launchAtLogin = autostart.value
    await api.saveSettings(settings.value)
  }
}
</script>

<template>
  <div v-if="settings" class="panel">
    <header class="head">
      <h1>拾遗设置</h1>
    </header>

    <!-- 保存成功的提示。绝对定位浮在面板上方，保存后自动关窗时仍能看见。 -->
    <Transition name="toast">
      <div v-if="toast" class="toast">已保存</div>
    </Transition>

    <div class="body">
      <section class="group">
        <h2>弹出时间</h2>
        <label class="row">
          <span class="lbl">开始时间</span>
          <select v-model.number="settings.startHour">
            <option v-for="h in HOURS" :key="h" :value="h">{{ String(h).padStart(2, '0') }}:00</option>
          </select>
        </label>
        <label class="row">
          <span class="lbl">结束时间</span>
          <select v-model.number="settings.endHour">
            <option v-for="h in HOURS" :key="h" :value="h">{{ String(h).padStart(2, '0') }}:00</option>
          </select>
        </label>
        <p v-if="!windowValid" class="warn">结束时间必须晚于开始时间</p>
        <label class="row">
          <span class="lbl">每天弹出次数</span>
          <input v-model.number="settings.popupsPerDay" type="number" min="1" max="12" />
        </label>
        <label class="row">
          <span class="lbl">卡片停留（秒）</span>
          <input v-model.number="settings.autoDismissSeconds" type="number" min="0" max="120" />
        </label>
        <label class="row check">
          <input v-model="settings.skipWeekend" type="checkbox" />
          <span class="lbl">周末不弹出</span>
        </label>
        <p class="hint">下一次弹出：{{ nextFireText }}</p>
      </section>

      <section class="group">
        <h2>外观</h2>
        <label class="row">
          <span class="lbl">卡片主题</span>
          <select v-model="settings.theme">
            <option v-for="t in themeOptions" :key="t.value" :value="t.value">{{ t.label }}</option>
          </select>
        </label>
        <p class="hint">选「随机」后每次弹出随机一种主题</p>
        <label class="row">
          <span class="lbl">界面样式</span>
          <select
            :value="settings.appearanceMode"
            @change="changeAppearance(($event.target as HTMLSelectElement).value)"
          >
            <option value="auto">跟随系统</option>
            <option value="light">亮色</option>
            <option value="dark">暗色</option>
          </select>
        </label>
      </section>

      <section class="group">
        <h2>快捷键与启动</h2>
        <div class="row">
          <span class="lbl">全局快捷键</span>
          <HotkeyRecorder v-model="settings.hotkey" />
        </div>
        <p class="hint">点「录制」后按下组合键；至少包含一个修饰键</p>
        <label class="row check">
          <input :checked="autostart" type="checkbox" @change="toggleAutostart" />
          <span class="lbl">开机自动启动</span>
        </label>
      </section>
    </div>

    <footer class="foot">
      <button class="save" :disabled="!windowValid" @click="save">保存</button>
    </footer>
  </div>
</template>

<style scoped>
.panel {
  display: flex;
  flex-direction: column;
  height: 100vh;
  background: var(--bg);
  /* 提示浮层以面板为定位基准 */
  position: relative;
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

/* 保存成功的浮层提示 */
.toast {
  position: absolute;
  left: 50%;
  top: 50%;
  transform: translate(-50%, -50%);
  background: var(--bg-elevated);
  color: var(--fg);
  border: 1px solid var(--border);
  border-radius: 10px;
  padding: 14px 26px;
  font-size: 14px;
  letter-spacing: 0.05em;
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.18);
  z-index: 10;
  pointer-events: none;
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
  padding: 4px 18px 12px;
}

.group {
  padding: 14px 0 6px;
  border-bottom: 1px solid var(--border);
}

.group:last-child {
  border-bottom: none;
}

.group h2 {
  margin: 0 0 10px;
  font-size: 12px;
  font-weight: 600;
  color: var(--fg-muted);
  letter-spacing: 0.06em;
}

.row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  margin-bottom: 9px;
}

.row.check {
  justify-content: flex-start;
  gap: 8px;
}

.lbl {
  font-size: 13px;
}

.hint {
  margin: -4px 0 10px;
  font-size: 11px;
  color: var(--fg-muted);
}

.warn {
  margin: -4px 0 9px;
  font-size: 11px;
  color: var(--accent);
}

.foot {
  padding: 10px 18px 14px;
  border-top: 1px solid var(--border);
}

.save {
  width: 100%;
  background: var(--accent);
  color: #fff;
  border: none;
  border-radius: var(--radius);
  padding: 8px;
  font-size: 13px;
  transition: opacity 0.15s ease;
}

.save:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}
</style>
