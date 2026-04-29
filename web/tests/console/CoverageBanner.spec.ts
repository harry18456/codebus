import { describe, expect, it } from 'vitest'
import { mount } from '@vue/test-utils'
import CoverageBanner from '~/components/console/CoverageBanner.vue'
import type {
  BudgetBannerState,
  BudgetWarningEvent,
  CoverageBannerEvent,
  CoverageSkipReason
} from '~/composables/useExplorerStream'

// CoverageBanner renders coverage_gaps and budget_warning events.
// Spec: openspec/changes/agent-console-p0/specs/agent-console/spec.md
//   "CoverageBanner renders coverage_gaps and budget_warning events"

function makeBudget(
  kind: 'tokens' | 'steps',
  current: number,
  budget: number,
  pct: number
): BudgetWarningEvent {
  return { kind, current, budget, pct }
}

function makeCoverage(
  skip_reason: CoverageSkipReason,
  gapsCount = 0,
  will_recurse = false
): CoverageBannerEvent {
  return {
    round: 0,
    gaps: Array.from({ length: gapsCount }, (_, i) => ({
      description: `gap ${i + 1}`,
      suggested_target: null
    })),
    will_recurse,
    skip_reason
  }
}

function bannerNodes(html: string): number {
  // Count occurrences of data-banner= and data-kind= attributes (each marks
  // exactly one rendered banner element in the component output).
  const banners = (html.match(/data-banner=/g) ?? []).length
  const kinds = (html.match(/data-kind=/g) ?? []).length
  return banners + kinds
}

describe('CoverageBanner', () => {
  it('all-null state renders nothing', () => {
    const budget: BudgetBannerState = {}
    const wrapper = mount(CoverageBanner, {
      props: { coverage: null, budget }
    })

    const html = wrapper.html()
    expect(html.includes('data-banner')).toBe(false)
    expect(html.includes('data-kind')).toBe(false)
    expect(bannerNodes(html)).toBe(0)
  })

  it('shows only the steps banner when both kinds are latched (steps priority)', () => {
    const budget: BudgetBannerState = {
      steps: makeBudget('steps', 4, 5, 0.8),
      tokens: makeBudget('tokens', 8000, 10000, 0.8)
    }
    const wrapper = mount(CoverageBanner, {
      props: { coverage: null, budget }
    })

    expect(bannerNodes(wrapper.html())).toBe(1)

    const stepsBanner = wrapper.find('[data-kind="steps"]')
    expect(stepsBanner.exists()).toBe(true)

    const tokensBanner = wrapper.find('[data-kind="tokens"]')
    expect(tokensBanner.exists()).toBe(false)
  })

  it('renders the tokens banner alone when only tokens kind is latched', () => {
    const budget: BudgetBannerState = {
      tokens: makeBudget('tokens', 8000, 10000, 0.8)
    }
    const wrapper = mount(CoverageBanner, {
      props: { coverage: null, budget }
    })

    expect(bannerNodes(wrapper.html())).toBe(1)
    const tokensBanner = wrapper.find('[data-kind="tokens"]')
    expect(tokensBanner.exists()).toBe(true)
    expect(tokensBanner.text()).toContain('8000')
    expect(tokensBanner.text()).toContain('10000')
  })

  it('coverage banner copy differs between no_gaps and budget_exhausted skip_reason', () => {
    const noGaps = mount(CoverageBanner, {
      props: { coverage: makeCoverage('no_gaps', 0, false), budget: {} }
    })
    const exhausted = mount(CoverageBanner, {
      props: {
        coverage: makeCoverage('budget_exhausted', 2, false),
        budget: {}
      }
    })

    const noGapsBanner = noGaps.find('[data-banner="coverage"]')
    const exhaustedBanner = exhausted.find('[data-banner="coverage"]')

    expect(noGapsBanner.exists()).toBe(true)
    expect(exhaustedBanner.exists()).toBe(true)
    expect(noGapsBanner.text()).not.toBe(exhaustedBanner.text())
  })

  it('coverage banner with max_depth_reached produces a distinct label', () => {
    const wrapper = mount(CoverageBanner, {
      props: {
        coverage: makeCoverage('max_depth_reached', 3, false),
        budget: {}
      }
    })
    const banner = wrapper.find('[data-banner="coverage"]')
    expect(banner.exists()).toBe(true)
    // Sanity: must mention the depth concept (not no_gaps / budget copy).
    expect(banner.text().toLowerCase()).toContain('depth')
  })

  it('coverage banner with will_recurse=true (skip_reason=null) mentions gap count', () => {
    const wrapper = mount(CoverageBanner, {
      props: { coverage: makeCoverage(null, 3, true), budget: {} }
    })
    const banner = wrapper.find('[data-banner="coverage"]')
    expect(banner.exists()).toBe(true)
    expect(banner.text()).toContain('3')
  })

  it('renders both a coverage banner and a steps budget banner together', () => {
    const wrapper = mount(CoverageBanner, {
      props: {
        coverage: makeCoverage('no_gaps', 0, false),
        budget: { steps: makeBudget('steps', 4, 5, 0.8) }
      }
    })

    const html = wrapper.html()
    expect(bannerNodes(html)).toBe(2)
    expect(wrapper.find('[data-banner="coverage"]').exists()).toBe(true)
    expect(wrapper.find('[data-kind="steps"]').exists()).toBe(true)
  })
})
