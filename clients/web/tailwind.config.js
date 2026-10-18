/** @type {import('tailwindcss').Config} */
export default {
  content: [
    "./components/**/*.{vue,js,ts}",
    "./layouts/**/*.vue",
    "./pages/**/*.vue",
    "./composables/**/*.{js,ts}",
    "./plugins/**/*.{js,ts}",
    "./app.vue",
    "./error.vue"
  ],
  darkMode: 'class',
  theme: {
    extend: {
      colors: {
        bg: 'var(--bg)',
        surface: {
          DEFAULT: 'var(--surface)',
          2: 'var(--surface-2)',
        },
        border: {
          DEFAULT: 'var(--border)',
          strong: 'var(--border-strong)',
        },
        text: {
          DEFAULT: 'var(--text)',
          muted: 'var(--text-muted)',
        },
        accent: {
          DEFAULT: 'var(--accent)',
          solid: 'var(--accent-solid)',
          fg: 'var(--accent-fg)',
        },
        ok: {
          DEFAULT: 'var(--ok)',
          bg: 'var(--ok-bg)',
        },
        warn: {
          DEFAULT: 'var(--warn)',
          bg: 'var(--warn-bg)',
        },
        bad: {
          DEFAULT: 'var(--bad)',
          bg: 'var(--bad-bg)',
        },
        info: {
          DEFAULT: 'var(--info)',
          bg: 'var(--info-bg)',
        },
        overlay: 'var(--overlay)',

        // Backwards compatibility aliases
        dark: {
          950: '#0b0c0e',
          900: 'var(--surface)',
          850: 'var(--surface-2)',
          800: 'var(--surface-2)',
          700: 'var(--border)',
          600: 'var(--text-muted)',
        },
        gold: {
          400: '#ffd043',
          500: 'var(--accent)',
          600: '#e0a800',
        },
      },
      boxShadow: {
        card: 'var(--shadow-card)',
      },
      fontFamily: {
        mono: ['"JetBrains Mono"', 'ui-monospace', 'SFMono-Regular', 'Menlo', 'Monaco', 'Consolas', 'monospace'],
        sans: ['Inter', 'system-ui', '-apple-system', 'sans-serif'],
      },
      borderRadius: {
        card: '8px',
        pill: '9999px',
      },
    },
  },
  plugins: [],
}
