import { describe, expect, it, vi } from 'vitest'

vi.mock('htmx.org', () => ({
  default: {},
}))

vi.mock('alpinejs', () => ({
  default: {
    store: () => null,
    start: () => {},
  },
}))

async function loadApp() {
  vi.resetModules()
  await import('./app.js')
}

describe('flash fallback', () => {
  it('creates fallback container when Alpine store is missing', async () => {
    document.body.innerHTML = ''
    await loadApp()

    ;(window as any).Alpine = undefined

    window.flash('Halo', 'success', { timeout: 0 })

    const container = document.getElementById('flash-fallback')
    expect(container).not.toBeNull()
    expect(container?.textContent).toContain('Halo')
  })
})

describe('global loading', () => {
  it('toggles loading element visibility', async () => {
    document.body.innerHTML = `
      <div id="global-loading" class="hidden opacity-0"></div>
    `
    await loadApp()

    window.loading.start()
    const el = document.getElementById('global-loading')
    expect(el?.classList.contains('hidden')).toBe(false)
    expect(el?.classList.contains('flex')).toBe(true)

    window.loading.stop()
    expect(window.loading.count).toBe(0)
  })
})
