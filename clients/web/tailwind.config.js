/**
 * Theme colours are CSS variables, which Tailwind cannot apply an opacity to (`border-border/60`
 * would silently be dropped). This turns "bg-surface-2/60" into a color-mix of the variable.
 */
const v = (name) => ({ opacityValue }) =>
  opacityValue === undefined || String(opacityValue).includes('var(')
    ? `var(--${name})`
    : `color-mix(in srgb, var(--${name}) ${Math.round(Number(opacityValue) * 100)}%, transparent)`;

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
        bg: v('bg'),
        surface: {
          DEFAULT: v('surface'),
          2: v('surface-2'),
        },
        border: {
          DEFAULT: v('border'),
          strong: v('border-strong'),
        },
        text: {
          DEFAULT: v('text'),
          muted: v('text-muted'),
        },
        accent: {
          DEFAULT: v('accent'),
          solid: v('accent-solid'),
          fg: v('accent-fg'),
        },
        ok: {
          DEFAULT: v('ok'),
          bg: v('ok-bg'),
        },
        warn: {
          DEFAULT: v('warn'),
          bg: v('warn-bg'),
        },
        bad: {
          DEFAULT: v('bad'),
          bg: v('bad-bg'),
        },
        info: {
          DEFAULT: v('info'),
          bg: v('info-bg'),
        },
        overlay: v('overlay'),

        // Backwards compatibility aliases
        dark: {
          950: '#0b0c0e',
          900: v('surface'),
          850: v('surface-2'),
          800: v('surface-2'),
          700: v('border'),
          600: v('text-muted'),
        },
        gold: {
          400: '#ffd043',
          500: v('accent'),
          600: '#e0a800',
        },
      },
      borderColor: {
        DEFAULT: v('border'),
      },
      boxShadow: {
        card: 'var(--shadow-card)',
      },
      fontFamily: {
        mono: ['"JetBrains Mono"', 'ui-monospace', 'SFMono-Regular', 'Menlo', 'Monaco', 'Consolas', 'monospace'],
        sans: ['Inter', 'system-ui', '-apple-system', 'sans-serif'],
      },
      borderRadius: {
        card: '14px',
        pill: '9999px',
      },
    },
  },
  plugins: [],
}
