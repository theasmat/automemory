// https://nuxt.com/docs/api/configuration/nuxt-config
export default defineNuxtConfig({
  ssr: false,
  telemetry: false,

  app: {
    head: {
      title: 'AutoMemory — Agent Memory Dashboard',
      htmlAttrs: {
        class: 'dark',
        lang: 'en',
      },
      meta: [
        { charset: 'utf-8' },
        { name: 'viewport', content: 'width=device-width, initial-scale=1' },
        { name: 'description', content: 'Local-first persistent memory engine and trajectory distillation for coding agents' },
      ],
      link: [
        { rel: 'icon', type: 'image/svg+xml', href: '/favicon.svg' },
      ],
      script: [
        {
          children: `(function(){try{var t=localStorage.getItem('am_theme');var isDark=t==='dark'||(!t||t==='system')&&window.matchMedia('(prefers-color-scheme: dark)').matches;if(isDark){document.documentElement.classList.add('dark');document.documentElement.classList.remove('light');document.documentElement.style.colorScheme='dark';}else{document.documentElement.classList.add('light');document.documentElement.classList.remove('dark');document.documentElement.style.colorScheme='light';}}catch(e){}})();`,
        },
      ],
    },
  },

  modules: [
    '@nuxtjs/tailwindcss',
    '@pinia/nuxt',
  ],

  // main.css holds the theme tokens (:root/.dark/.light) and base styles; without this the module
  // loads only its default stylesheet and every var(--bg) etc. is undefined.
  tailwindcss: {
    cssPath: '~/assets/css/main.css',
  },

  nitro: {
    // `pnpm dev` serves the UI on :3000 while the daemon (API + WebSocket) listens on :4545.
    // Without this the page calls :3000/api, gets the HTML shell back, and shows "isn't running".
    // The daemon only accepts its own origin, so the proxy presents itself as that origin.
    devProxy: {
      '/api': {
        target: 'http://127.0.0.1:4545/api',
        changeOrigin: true,
        headers: { Origin: 'http://127.0.0.1:4545' },
      },
      '/ws': {
        target: 'ws://127.0.0.1:4545/ws',
        ws: true,
        changeOrigin: true,
        headers: { Origin: 'http://127.0.0.1:4545' },
      },
    },
    output: {
      publicDir: 'dist',
    },
    prerender: {
      crawlLinks: true,
      routes: [
        '/',
        '/overview',
        '/memories',
        '/replay',
        '/traces',
        '/failures',
        '/proposals',
        '/guardrails',
      ],
    },
  },

  typescript: {
    strict: true,
    shim: false,
  },

  compatibilityDate: '2025-01-01',
});
