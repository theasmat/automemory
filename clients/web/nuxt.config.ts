// https://nuxt.com/docs/api/configuration/nuxt-config
export default defineNuxtConfig({
  ssr: false,
  telemetry: false,

  app: {
    head: {
      title: 'AutoMemory — Agent Memory Dashboard',
      meta: [
        { charset: 'utf-8' },
        { name: 'viewport', content: 'width=device-width, initial-scale=1' },
        { name: 'description', content: 'Local-first persistent memory engine and trajectory distillation for coding agents' },
      ],
      link: [
        { rel: 'icon', type: 'image/svg+xml', href: '/favicon.svg' },
      ],
    },
  },

  modules: [
    '@nuxtjs/tailwindcss',
    '@pinia/nuxt',
  ],

  nitro: {
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
