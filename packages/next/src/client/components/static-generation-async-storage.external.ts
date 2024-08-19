import type { AsyncLocalStorage } from 'async_hooks'
import type { IncrementalCache } from '../../server/lib/incremental-cache'
import type { DynamicServerError } from './hooks-server-context'
import type { FetchMetrics } from '../../server/base-http'
import type { Revalidate } from '../../server/lib/revalidate'
import type { PrerenderState } from '../../server/app-render/dynamic-rendering'

// Share the instance module in the next-shared layer
import { staticGenerationAsyncStorage } from './static-generation-async-storage-instance' with { 'turbopack-transition': 'next-shared' }

export interface StaticGenerationStore {
  readonly isStaticGeneration: boolean
  readonly pagePath?: string
  readonly urlPathname: string
  readonly incrementalCache?: IncrementalCache
  readonly isOnDemandRevalidate?: boolean
  readonly isPrerendering?: boolean
  readonly isRevalidate?: boolean
  readonly isUnstableCacheCallback?: boolean

  // When this exists (is not null) it means we are in a Prerender
  prerenderState: null | PrerenderState

  forceDynamic?: boolean
  fetchCache?:
    | 'only-cache'
    | 'force-cache'
    | 'default-cache'
    | 'force-no-store'
    | 'default-no-store'
    | 'only-no-store'

  revalidate?: Revalidate
  forceStatic?: boolean
  dynamicShouldError?: boolean
  pendingRevalidates?: Record<string, Promise<any>>

  dynamicUsageDescription?: string
  dynamicUsageStack?: string
  dynamicUsageErr?: DynamicServerError

  nextFetchId?: number
  pathWasRevalidated?: boolean

  tags?: string[]

  revalidatedTags?: string[]
  fetchMetrics?: FetchMetrics

  isDraftMode?: boolean
  isUnstableNoStore?: boolean

  requestEndedState?: { ended?: boolean }
}

export type StaticGenerationAsyncStorage =
  AsyncLocalStorage<StaticGenerationStore>

export { staticGenerationAsyncStorage }
