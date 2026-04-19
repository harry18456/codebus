// https://nuxt.com/docs/api/configuration/nuxt-config
export default defineNuxtConfig({
  modules: ['@nuxtjs/tailwindcss'],
  devtools: { enabled: true },
  typescript: {
    strict: true,
    typeCheck: false
  },
  ssr: false,
  srcDir: 'app/',
  app: {
    head: {
      title: 'CodeBus'
    }
  }
})
