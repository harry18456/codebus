// https://nuxt.com/docs/api/configuration/nuxt-config
export default defineNuxtConfig({
  // Lock Nitro to current behavior; without this Nitro emits a WARN at
  // dev start and may flip default behavior on future minor upgrades.
  // Bump this when intentionally adopting newer Nitro defaults.
  compatibilityDate: '2026-05-02',
  modules: ['@nuxtjs/tailwindcss', '@nuxtjs/mdc'],
  devtools: { enabled: true },
  typescript: {
    strict: true,
    typeCheck: false
  },
  ssr: false,
  srcDir: 'app/',
  app: {
    head: {
      title: 'CodeBus',
      link: [
        {
          rel: 'preconnect',
          href: 'https://fonts.googleapis.com'
        },
        {
          rel: 'preconnect',
          href: 'https://fonts.gstatic.com',
          crossorigin: ''
        },
        {
          rel: 'stylesheet',
          href: 'https://fonts.googleapis.com/css2?family=Inter:wght@400;500;600;700&family=Noto+Sans+TC:wght@400;500;600;700&family=JetBrains+Mono:wght@400;500;600&display=swap'
        }
      ]
    }
  }
})
