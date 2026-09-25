import Alpine from 'alpinejs'
import htmx from 'htmx.org'

// Setup Alpine
window.Alpine = Alpine

document.addEventListener('alpine:init', () => {
  Alpine.store('flash', {
    items: [],
    add(message, type = 'success', options = {}) {
      const id =
        typeof crypto !== 'undefined' && crypto.randomUUID
          ? crypto.randomUUID()
          : `${Date.now()}-${Math.random().toString(16).slice(2)}`
      const timeout = Number.isFinite(options.timeout) ? options.timeout : 3500
      const item = {
        id,
        message,
        type,
        timeout,
        createdAt: Date.now(),
      }

      this.items.push(item)

      if (timeout > 0) {
        setTimeout(() => this.remove(id), timeout)
      }
    },
    remove(id) {
      this.items = this.items.filter((t) => t.id !== id)
    },
  })
})

function fallbackFlash(message, type = 'success', options = {}) {
  let container = document.getElementById('flash-fallback')
  if (!container) {
    container = document.createElement('div')
    container.id = 'flash-fallback'
    container.className =
      'fixed top-4 right-4 z-50 flex flex-col gap-3 max-w-sm w-[calc(100vw-2rem)]'
    document.body.appendChild(container)
  }

  const toast = document.createElement('div')
  const tone =
    type === 'success'
      ? 'border-emerald-500'
      : type === 'error'
      ? 'border-rose-500'
      : type === 'warning'
      ? 'border-amber-500'
      : 'border-sky-500'

  toast.className = `bg-white border-l-4 ${tone} shadow-lg rounded-lg px-4 py-3 text-sm text-gray-800`
  toast.textContent = message
  container.appendChild(toast)

  const timeout = Number.isFinite(options.timeout) ? options.timeout : 3500
  if (timeout > 0) {
    setTimeout(() => {
      toast.remove()
      if (!container.children.length) {
        container.remove()
      }
    }, timeout)
  }
}

window.flash = (message, type = 'success', options = {}) => {
  if (window.Alpine && window.Alpine.store) {
    const store = window.Alpine.store('flash')
    if (store && typeof store.add === 'function') {
      store.add(message, type, options)
      return
    }
  }
  fallbackFlash(message, type, options)
}

window.addEventListener('flash', (event) => {
  const detail = event.detail || {}
  window.flash(detail.message || 'Sukses', detail.type || 'success', detail)
})

const loadingEl = () => document.getElementById('global-loading')

function showGlobalLoading() {
  const el = loadingEl()
  if (!el) return
  el.classList.remove('hidden')
  el.classList.add('flex')
  requestAnimationFrame(() => {
    el.classList.remove('opacity-0')
  })
}

function hideGlobalLoading() {
  const el = loadingEl()
  if (!el) return
  el.classList.add('opacity-0')
  setTimeout(() => {
    el.classList.add('hidden')
    el.classList.remove('flex')
  }, 200)
}

window.loading = {
  count: 0,
  start() {
    this.count += 1
    if (this.count === 1) showGlobalLoading()
  },
  stop() {
    this.count = Math.max(0, this.count - 1)
    if (this.count === 0) hideGlobalLoading()
  },
  reset() {
    this.count = 0
    hideGlobalLoading()
  },
}

document.addEventListener('htmx:beforeRequest', () => window.loading.start())
document.addEventListener('htmx:afterRequest', () => window.loading.stop())
document.addEventListener('htmx:responseError', () => window.loading.stop())
document.addEventListener('htmx:afterSwap', () => window.loading.stop())
document.addEventListener('htmx:afterSettle', () => window.loading.stop())

document.addEventListener('submit', (event) => {
  const form = event.target
  if (!(form instanceof HTMLFormElement)) return
  if (form.closest('[data-loading="off"]')) return
  // Submitting to another browsing context leaves the current page usable, so
  // there is no navigation on this tab whose completion could clear the loader.
  if (form.target && form.target !== '_self') return
  if (
    form.hasAttribute('hx-get') ||
    form.hasAttribute('hx-post') ||
    form.hasAttribute('hx-put') ||
    form.hasAttribute('hx-patch') ||
    form.hasAttribute('hx-delete')
  ) {
    return
  }
  window.loading.start()
})

document.addEventListener('click', (event) => {
  const link = event.target.closest('a')
  if (!link) return
  if (link.closest('[data-loading="off"]')) return
  if (link.hasAttribute('download')) return
  if (link.target && link.target !== '_self') return
  if (link.hasAttribute('hx-get') || link.hasAttribute('hx-post')) return
  const href = link.getAttribute('href') || ''
  if (!href || href.startsWith('#') || href.startsWith('mailto:') || href.startsWith('tel:')) return
  if (href.startsWith('http') && !href.startsWith(window.location.origin)) return
  window.loading.start()
})

const originalFetch = window.fetch
if (originalFetch) {
  window.fetch = (...args) => {
    window.loading.start()
    return originalFetch(...args)
      .catch((error) => {
        throw error
      })
      .finally(() => {
        window.loading.stop()
      })
  }
}

Alpine.start()

// Setup HTMX
window.htmx = htmx

// Export jika perlu
export { Alpine, htmx }
