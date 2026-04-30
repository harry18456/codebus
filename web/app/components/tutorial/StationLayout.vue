<script setup lang="ts">
// Shell that renders frontmatter chrome (title / index / duration /
// degraded badge) and slots station body. The parent page passes the
// gray-matter-parsed frontmatter object as a single prop; missing
// required fields (station_id, title) are handled at the page level
// before this component mounts (per spec scenario `Missing required
// frontmatter field triggers safe fallback`).

export interface StationFrontmatter {
  station_id: string
  station_index: number
  title: string
  duration_minutes?: number
  workspace_type?: string
  repo_name?: string
  task?: string
  generated_at?: string
  related_stations?: string[]
  required_checks?: string[]
  degraded?: boolean
  schema_version?: number
}

const props = defineProps<{
  frontmatter: StationFrontmatter
  totalStations: number
}>()
</script>

<template>
  <article class="px-12 py-10 max-w-[760px] mx-auto text-text-base">
    <header class="mb-6">
      <div
        class="flex flex-wrap items-center gap-3 mb-3 font-mono text-[10.5px] text-text-mute"
      >
        <span class="px-2 py-[2px] rounded bg-surface-2 text-text-dim">
          站 {{ props.frontmatter.station_index }} / {{ props.totalStations }}
        </span>
        <span
          v-if="props.frontmatter.duration_minutes"
          class="px-2 py-[2px] rounded bg-surface-2 text-text-dim"
        >
          {{ props.frontmatter.duration_minutes }} 分鐘
        </span>
        <span
          v-if="props.frontmatter.related_stations?.length"
          class="text-text-mute"
        >
          related: {{ props.frontmatter.related_stations.join(', ') }}
        </span>
        <span
          v-if="props.frontmatter.degraded"
          data-testid="degraded-badge"
          class="ml-auto px-2 py-[2px] rounded font-mono text-[11px] bg-orange/20 text-orange"
        >
          ⚠ 本站產出失敗，請重跑
        </span>
      </div>
      <div class="flex items-start gap-3 flex-wrap">
        <h1 class="text-[32px] font-bold tracking-tight leading-tight flex-1 min-w-0">
          {{ props.frontmatter.title }}
        </h1>
        <div class="flex items-center gap-2 mt-2">
          <slot name="header-actions" />
        </div>
      </div>
    </header>
    <div class="text-[15px] leading-[1.75]">
      <slot />
    </div>
  </article>
</template>
