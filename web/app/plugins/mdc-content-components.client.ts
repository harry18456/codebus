// Explicit registration for the three R-01 mdc content components.
//
// `@nuxtjs/mdc` is supposed to auto-mount `app/components/content/`
// into the markdown renderer, but in our Nuxt 4 + srcDir='app/' setup
// the manifest does not pick the directory up reliably — every
// `<Checkpoint>` / `<Quiz>` / `<QAEntry>` tag falls back to Vue's
// global resolveComponent and warns "Failed to resolve component"
// (the markdown then renders as inert `<div>`s, so Checkpoint /
// Quiz never fire setCheckpoint / setQuizAnswer and progress.json
// stays empty even after the user "completes" a station — the
// symptom that surfaced this bug during manual smoke).
//
// This plugin sidesteps the mdc manifest by registering the three
// components on the global Vue app. We register each under both its
// canonical name and the lowercase-folded alias because Vue
// normalises HTML tag names (`<QAEntry>` → `qaentry` → `QaEntry`)
// before resolution; the alias catches that path.

import Checkpoint from '~/components/content/Checkpoint.vue'
import QAEntry from '~/components/content/QAEntry.vue'
import Quiz from '~/components/content/Quiz.vue'

export default defineNuxtPlugin((nuxtApp) => {
  nuxtApp.vueApp.component('Checkpoint', Checkpoint)
  nuxtApp.vueApp.component('Quiz', Quiz)
  nuxtApp.vueApp.component('QAEntry', QAEntry)
  nuxtApp.vueApp.component('QaEntry', QAEntry)
})
