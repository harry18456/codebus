import { defineConfig } from 'vitest/config'
import vue from '@vitejs/plugin-vue'
import { fileURLToPath, URL } from 'node:url'

// Vitest is bootstrapped by change agent-console-p0 (apply-time ingest).
// Avoid pulling @nuxt/test-utils — it boots a full Nuxt runtime which is
// out of scope for P0 unit/integration tests.
export default defineConfig({
  plugins: [vue()],
  resolve: {
    alias: {
      '~': fileURLToPath(new URL('./app', import.meta.url)),
      '@': fileURLToPath(new URL('./app', import.meta.url))
    }
  },
  test: {
    environment: 'happy-dom',
    globals: true,
    setupFiles: ['tests/setup.ts'],
    include: ['tests/**/*.spec.ts'],
    coverage: {
      provider: 'v8',
      reporter: ['text', 'html'],
      include: ['app/composables/**', 'app/components/**', 'app/pages/**'],
      exclude: ['app/components/content/**']
    }
  }
})
