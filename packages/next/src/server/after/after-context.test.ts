import { DetachedPromise } from '../../lib/detached-promise'
import { AsyncLocalStorage } from 'async_hooks'

import type { RequestStore } from '../../client/components/request-async-storage.external'
import type { AfterContext } from './after-context'

describe('createAfterContext', () => {
  // 'async-local-storage.ts' needs `AsyncLocalStorage` on `globalThis` at import time,
  // so we have to do some contortions here to set it up before running anything else
  type RASMod =
    typeof import('../../client/components/request-async-storage.external')
  type AfterMod = typeof import('./after')
  type AfterContextMod = typeof import('./after-context')

  let requestAsyncStorage: RASMod['requestAsyncStorage']
  let createAfterContext: AfterContextMod['createAfterContext']
  let after: AfterMod['unstable_after']

  beforeAll(async () => {
    // @ts-expect-error
    globalThis.AsyncLocalStorage = AsyncLocalStorage

    const RASMod = await import(
      '../../client/components/request-async-storage.external'
    )
    requestAsyncStorage = RASMod.requestAsyncStorage

    const AfterContextMod = await import('./after-context')
    createAfterContext = AfterContextMod.createAfterContext

    const AfterMod = await import('./after')
    after = AfterMod.unstable_after
  })

  const createRun =
    (afterContext: AfterContext, requestStore: RequestStore) =>
    <T>(cb: () => T): T => {
      return afterContext.run(requestStore, () =>
        requestAsyncStorage.run(requestStore, cb)
      )
    }

  it('runs after() callbacks from a run() callback that resolves', async () => {
    const waitUntilPromises: Promise<unknown>[] = []
    const waitUntil = jest.fn((promise) => waitUntilPromises.push(promise))

    let onCloseCallback: (() => void) | undefined = undefined
    const onClose = jest.fn((cb) => {
      onCloseCallback = cb
    })

    const afterContext = createAfterContext({
      waitUntil,
      onClose,
      cacheScope: undefined,
    })

    const requestStore = createMockRequestStore(afterContext)
    const run = createRun(afterContext, requestStore)

    // ==================================

    const promise0 = new DetachedPromise<string>()

    const promise1 = new DetachedPromise<string>()
    const afterCallback1 = jest.fn(() => promise1.promise)

    const promise2 = new DetachedPromise<string>()
    const afterCallback2 = jest.fn(() => promise2.promise)

    await run(async () => {
      after(promise0.promise)
      expect(onClose).not.toHaveBeenCalled() // we don't need onClose for bare promises
      expect(waitUntil).toHaveBeenCalledTimes(1)

      await Promise.resolve(null)

      after(afterCallback1)
      expect(waitUntil).toHaveBeenCalledTimes(2) // just runCallbacksOnClose

      await Promise.resolve(null)

      after(afterCallback2)
      expect(waitUntil).toHaveBeenCalledTimes(2) // should only `waitUntil(this.runCallbacksOnClose())` once for all callbacks
    })

    expect(onClose).toHaveBeenCalledTimes(1)
    expect(afterCallback1).not.toHaveBeenCalled()
    expect(afterCallback2).not.toHaveBeenCalled()

    // the response is done.
    onCloseCallback!()
    await Promise.resolve(null)

    expect(afterCallback1).toHaveBeenCalledTimes(1)
    expect(afterCallback2).toHaveBeenCalledTimes(1)
    expect(waitUntil).toHaveBeenCalledTimes(2)

    promise0.resolve('0')
    promise1.resolve('1')
    promise2.resolve('2')

    const results = await Promise.all(waitUntilPromises)
    expect(results).toEqual([
      '0', // promises are passed to waitUntil as is
      undefined, // callbacks all get collected into a big void promise
    ])
  })

  it('runs after() callbacks from a run() callback that throws', async () => {
    const waitUntilPromises: Promise<unknown>[] = []
    const waitUntil = jest.fn((promise) => waitUntilPromises.push(promise))

    let onCloseCallback: (() => void) | undefined = undefined
    const onClose = jest.fn((cb) => {
      onCloseCallback = cb
    })

    const afterContext = createAfterContext({
      waitUntil,
      onClose,
      cacheScope: undefined,
    })

    const requestStore = createMockRequestStore(afterContext)

    const run = createRun(afterContext, requestStore)

    // ==================================

    const promise1 = new DetachedPromise<string>()
    const afterCallback1 = jest.fn(() => promise1.promise)

    await run(async () => {
      after(afterCallback1)
      throw new Error('boom!')
    }).catch(() => {})

    // runCallbacksOnClose
    expect(waitUntil).toHaveBeenCalledTimes(1)
    expect(onClose).toHaveBeenCalledTimes(1)

    expect(afterCallback1).not.toHaveBeenCalled()

    // the response is done.
    onCloseCallback!()
    await Promise.resolve(null)

    expect(afterCallback1).toHaveBeenCalledTimes(1)
    expect(waitUntil).toHaveBeenCalledTimes(1)

    promise1.resolve('1')

    const results = await Promise.all(waitUntilPromises)
    expect(results).toEqual([undefined])
  })

  it('runs after() callbacks from a run() callback that streams', async () => {
    const waitUntilPromises: Promise<unknown>[] = []
    const waitUntil = jest.fn((promise) => waitUntilPromises.push(promise))

    let onCloseCallback: (() => void) | undefined = undefined
    const onClose = jest.fn((cb) => {
      onCloseCallback = cb
    })

    const afterContext = createAfterContext({
      waitUntil,
      onClose,
      cacheScope: undefined,
    })

    const requestStore = createMockRequestStore(afterContext)

    const run = createRun(afterContext, requestStore)

    // ==================================

    const promise1 = new DetachedPromise<string>()
    const afterCallback1 = jest.fn(() => promise1.promise)

    const promise2 = new DetachedPromise<string>()
    const afterCallback2 = jest.fn(() => promise2.promise)

    const streamStarted = new DetachedPromise<void>()

    const stream = run(() => {
      return new ReadableStream<string>({
        async start(controller) {
          await streamStarted.promise // block the stream to start it manually later

          const delay = () =>
            new Promise<void>((resolve) => setTimeout(resolve, 50))

          after(afterCallback1)
          controller.enqueue('one')
          await delay()
          expect(waitUntil).toHaveBeenCalledTimes(1) // runCallbacksOnClose

          after(afterCallback2)
          controller.enqueue('two')
          await delay()
          expect(waitUntil).toHaveBeenCalledTimes(1) // runCallbacksOnClose

          await delay()
          controller.close()
        },
      })
    })

    expect(onClose).not.toHaveBeenCalled() // no after()s executed yet
    expect(afterCallback1).not.toHaveBeenCalled()
    expect(afterCallback2).not.toHaveBeenCalled()

    // start the stream and consume it, which'll execute the after()s.
    {
      streamStarted.resolve()
      const reader = stream.getReader()
      while (true) {
        const chunk = await reader.read()
        if (chunk.done) {
          break
        }
      }
    }

    // runCallbacksOnClose
    expect(onClose).toHaveBeenCalledTimes(1)
    expect(waitUntil).toHaveBeenCalledTimes(1)

    expect(afterCallback1).not.toHaveBeenCalled()
    expect(afterCallback2).not.toHaveBeenCalled()

    // the response is done.
    onCloseCallback!()
    await Promise.resolve(null)

    expect(afterCallback1).toHaveBeenCalledTimes(1)
    expect(afterCallback2).toHaveBeenCalledTimes(1)
    expect(waitUntil).toHaveBeenCalledTimes(1)

    promise1.resolve('1')
    promise2.resolve('2')

    const results = await Promise.all(waitUntilPromises)
    expect(results).toEqual([undefined])
  })

  it('runs after() callbacks added within an after()', async () => {
    const waitUntilPromises: Promise<unknown>[] = []
    const waitUntil = jest.fn((promise) => waitUntilPromises.push(promise))

    let onCloseCallback: (() => void) | undefined = undefined
    const onClose = jest.fn((cb) => {
      onCloseCallback = cb
    })

    const afterContext = createAfterContext({
      waitUntil,
      onClose,
      cacheScope: undefined,
    })

    const requestStore = createMockRequestStore(afterContext)
    const run = createRun(afterContext, requestStore)

    // ==================================

    const promise1 = new DetachedPromise<string>()
    const afterCallback1 = jest.fn(async () => {
      await promise1.promise
      after(afterCallback2)
    })

    const promise2 = new DetachedPromise<string>()
    const afterCallback2 = jest.fn(() => promise2.promise)

    await run(async () => {
      after(afterCallback1)
      expect(onClose).toHaveBeenCalledTimes(1)
      expect(waitUntil).toHaveBeenCalledTimes(1) // just runCallbacksOnClose
    })

    expect(onClose).toHaveBeenCalledTimes(1)
    expect(afterCallback1).not.toHaveBeenCalled()
    expect(afterCallback2).not.toHaveBeenCalled()

    // the response is done.
    onCloseCallback!()
    await Promise.resolve(null)

    expect(afterCallback1).toHaveBeenCalledTimes(1)
    expect(afterCallback2).toHaveBeenCalledTimes(0)
    expect(waitUntil).toHaveBeenCalledTimes(1)

    promise1.resolve('1')
    await Promise.resolve(null)

    expect(afterCallback1).toHaveBeenCalledTimes(1)
    expect(afterCallback2).toHaveBeenCalledTimes(1)
    expect(waitUntil).toHaveBeenCalledTimes(1)
    promise2.resolve('2')

    const results = await Promise.all(waitUntilPromises)
    expect(results).toEqual([
      undefined, // callbacks all get collected into a big void promise
    ])
  })

  it('does not hang forever if onClose failed', async () => {
    const waitUntilPromises: Promise<unknown>[] = []
    const waitUntil = jest.fn((promise) => waitUntilPromises.push(promise))

    const onClose = jest.fn(() => {
      throw new Error('onClose is broken for some reason')
    })

    const afterContext = createAfterContext({
      waitUntil,
      onClose,
      cacheScope: undefined,
    })

    const requestStore = createMockRequestStore(afterContext)

    const run = createRun(afterContext, requestStore)

    // ==================================

    const afterCallback1 = jest.fn()

    await run(async () => {
      after(afterCallback1)
    })

    expect(waitUntil).toHaveBeenCalledTimes(1) // runCallbacksOnClose
    expect(onClose).toHaveBeenCalledTimes(1)
    expect(afterCallback1).not.toHaveBeenCalled()

    // if we didn't properly reject the runCallbacksOnClose promise, this should hang forever, and get killed by jest.
    const results = await Promise.allSettled(waitUntilPromises)
    expect(results).toEqual([
      { status: 'rejected', value: undefined, reason: expect.anything() },
    ])
  })

  it('runs all after() callbacks even if some of them threw', async () => {
    const waitUntilPromises: Promise<unknown>[] = []
    const waitUntil = jest.fn((promise) => waitUntilPromises.push(promise))

    let onCloseCallback: (() => void) | undefined = undefined
    const onClose = jest.fn((cb) => {
      onCloseCallback = cb
    })

    const afterContext = createAfterContext({
      waitUntil,
      onClose,
      cacheScope: undefined,
    })

    const requestStore = createMockRequestStore(afterContext)

    // ==================================

    const promise1 = new DetachedPromise<string>()
    const afterCallback1 = jest.fn(() => promise1.promise)

    const afterCallback2 = jest.fn(() => {
      throw new Error('2')
    })

    const promise3 = new DetachedPromise<string>()
    const afterCallback3 = jest.fn(() => promise3.promise)

    requestAsyncStorage.run(requestStore, () =>
      afterContext.run(requestStore, () => {
        after(afterCallback1)
        after(afterCallback2)
        after(afterCallback3)
      })
    )

    expect(afterCallback1).not.toHaveBeenCalled()
    expect(afterCallback2).not.toHaveBeenCalled()
    expect(afterCallback3).not.toHaveBeenCalled()
    expect(waitUntil).toHaveBeenCalledTimes(1)

    // the response is done.
    onCloseCallback!()
    await Promise.resolve(null)

    expect(afterCallback1).toHaveBeenCalledTimes(1)
    expect(afterCallback2).toHaveBeenCalledTimes(1)
    expect(afterCallback3).toHaveBeenCalledTimes(1)
    expect(waitUntil).toHaveBeenCalledTimes(1)

    promise1.reject(new Error('1'))
    promise3.resolve('3')

    const results = await Promise.all(waitUntilPromises)
    expect(results).toEqual([undefined])
  })

  it('throws from after() if waitUntil is not provided', async () => {
    const waitUntil = undefined
    const onClose = jest.fn()

    const afterContext = createAfterContext({
      waitUntil,
      onClose,
      cacheScope: undefined,
    })

    const requestStore = createMockRequestStore(afterContext)

    const run = createRun(afterContext, requestStore)

    // ==================================

    const afterCallback1 = jest.fn()

    expect(() =>
      run(() => {
        after(afterCallback1)
      })
    ).toThrow(/`waitUntil` is not available in the current environment/)

    expect(onClose).not.toHaveBeenCalled()
    expect(afterCallback1).not.toHaveBeenCalled()
  })

  it('throws from after() if onClose is not provided', async () => {
    const waitUntilPromises: Promise<unknown>[] = []
    const waitUntil = jest.fn((promise) => waitUntilPromises.push(promise))

    const onClose = undefined

    const afterContext = createAfterContext({
      waitUntil,
      onClose,
      cacheScope: undefined,
    })

    const requestStore = createMockRequestStore(afterContext)

    const run = createRun(afterContext, requestStore)

    // ==================================

    const afterCallback1 = jest.fn()

    expect(() =>
      run(() => {
        after(afterCallback1)
      })
    ).toThrow(/Missing `onClose` implementation/)

    expect(waitUntil).not.toHaveBeenCalled()
    expect(afterCallback1).not.toHaveBeenCalled()
  })
})

const createMockRequestStore = (afterContext: AfterContext): RequestStore => {
  const partialStore: Partial<RequestStore> = {
    afterContext: afterContext,
    assetPrefix: '',
    reactLoadableManifest: {},
    draftMode: undefined,
  }

  return new Proxy(partialStore, {
    get(target, key) {
      if (key in target) {
        return target[key as keyof typeof target]
      }
      throw new Error(
        `RequestStore property not mocked: '${typeof key === 'symbol' ? key.toString() : key}'`
      )
    },
  }) as RequestStore
}
