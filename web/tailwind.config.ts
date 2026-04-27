import type { Config } from 'tailwindcss'

// Design tokens mirror `design/v1/tokens.css` 1:1 — that file is the source of
// truth. When updating values, update both files in lockstep, or the
// `Design tokens originate from a single source` invariant breaks.
export default {
  content: [
    './app/components/**/*.{vue,ts}',
    './app/layouts/**/*.{vue,ts}',
    './app/pages/**/*.{vue,ts}',
    './app/composables/**/*.ts',
    './app/app.vue'
  ],
  theme: {
    extend: {
      colors: {
        surface: {
          0: '#0b0d10',
          1: '#13161b',
          2: '#191d24',
          3: '#1f242c',
          4: '#262c35'
        },
        border: {
          base: '#262c35',
          soft: '#1c2129'
        },
        text: {
          base: '#e6e8eb',
          dim: '#9aa3ae',
          mute: '#636b76'
        },
        accent: 'oklch(72% 0.12 210)',
        'accent-2': 'oklch(72% 0.12 85)',
        green: 'oklch(72% 0.12 155)',
        yellow: 'oklch(78% 0.13 95)',
        orange: 'oklch(73% 0.15 55)',
        red: 'oklch(68% 0.17 25)',
        purple: 'oklch(70% 0.16 295)'
      },
      fontFamily: {
        sans: ['Inter', '"Noto Sans TC"', 'system-ui', 'sans-serif'],
        mono: ['"JetBrains Mono"', 'ui-monospace', 'Menlo', 'monospace']
      }
    }
  }
} satisfies Config
