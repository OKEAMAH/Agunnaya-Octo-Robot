import React from 'react'
import { fillCacheWithNewSubTreeData } from './fill-cache-with-new-subtree-data'
import type { CacheNode } from '../../../shared/lib/app-router-context.shared-runtime'
import type { FlightData } from '../../../server/app-render/types'

const getFlightData = (): FlightData => {
  return [
    [
      'children',
      'linking',
      'children',
      'about',
      [
        'about',
        {
          children: ['', {}],
        },
      ],
      ['about', {}, <h1>SubTreeData Injected!</h1>],
      <>
        <title>Head Injected!</title>
      </>,
    ],
  ]
}

describe('fillCacheWithNewSubtreeData', () => {
  it('should apply rsc and head property', () => {
    const cache: CacheNode = {
      lazyData: null,
      rsc: null,
      prefetchRsc: null,
      head: null,
      prefetchHead: null,
      loading: null,
      parallelRoutes: new Map(),
      lazyDataResolved: false,
    }
    const existingCache: CacheNode = {
      lazyData: null,
      rsc: <>Root layout</>,
      prefetchRsc: null,
      head: null,
      prefetchHead: null,
      lazyDataResolved: false,
      loading: null,
      parallelRoutes: new Map([
        [
          'children',
          new Map([
            [
              'linking',
              {
                lazyData: null,
                rsc: <>Linking</>,
                prefetchRsc: null,
                head: null,
                prefetchHead: null,
                lazyDataResolved: false,
                loading: null,
                parallelRoutes: new Map([
                  [
                    'children',
                    new Map([
                      [
                        '',
                        {
                          lazyData: null,
                          rsc: <>Page</>,
                          prefetchRsc: null,
                          head: null,
                          prefetchHead: null,
                          lazyDataResolved: false,
                          loading: null,
                          parallelRoutes: new Map(),
                        },
                      ],
                    ]),
                  ],
                ]),
              },
            ],
          ]),
        ],
      ]),
    }

    const flightData = getFlightData()

    if (typeof flightData === 'string') {
      throw new Error('invalid flight data')
    }

    // Mirrors the way router-reducer values are passed in.
    const flightDataPath = flightData[0]

    fillCacheWithNewSubTreeData(cache, existingCache, flightDataPath)

    const expectedCache: CacheNode = {
      lazyData: null,
      rsc: null,
      prefetchRsc: null,
      head: null,
      prefetchHead: null,
      lazyDataResolved: false,
      loading: null,
      parallelRoutes: new Map([
        [
          'children',
          new Map([
            [
              'linking',
              {
                lazyData: null,
                rsc: <>Linking</>,
                prefetchRsc: null,
                head: null,
                prefetchHead: null,
                lazyDataResolved: false,
                loading: null,
                parallelRoutes: new Map([
                  [
                    'children',
                    new Map([
                      // TODO-APP: this segment should be preserved when creating the new cache
                      [
                        '',
                        {
                          lazyData: null,
                          rsc: <>Page</>,
                          prefetchRsc: null,
                          head: null,
                          prefetchHead: null,
                          lazyDataResolved: false,
                          loading: null,
                          parallelRoutes: new Map(),
                        },
                      ],
                      [
                        'about',
                        {
                          lazyData: null,
                          lazyDataResolved: false,
                          head: null,
                          prefetchHead: null,
                          loading: null,
                          parallelRoutes: new Map([
                            [
                              'children',
                              new Map([
                                [
                                  '',
                                  {
                                    lazyData: null,
                                    rsc: null,
                                    prefetchRsc: null,
                                    parallelRoutes: new Map(),
                                    prefetchHead: null,
                                    loading: null,
                                    head: (
                                      <>
                                        <title>Head Injected!</title>
                                      </>
                                    ),
                                  },
                                ],
                              ]),
                            ],
                          ]),
                          rsc: <h1>SubTreeData Injected!</h1>,
                          prefetchRsc: null,
                        },
                      ],
                    ]),
                  ],
                ]),
              },
            ],
          ]),
        ],
      ]),
    }

    expect(cache).toMatchObject(expectedCache)
  })
})
