import { nextTestSetup } from 'e2e-utils'
import webdriver from 'next-webdriver'
import {
  hasRedbox,
  getRedboxSource,
  getRedboxDescription,
  check,
} from 'next-test-utils'
import stripAnsi from 'strip-ansi'

describe('fetch failures have good stack traces in edge runtime', () => {
  const { next, isNextStart, isNextDev, skipped } = nextTestSetup({
    files: __dirname,
    // don't have access to runtime logs on deploy
    skipDeployment: true,
  })

  if (skipped) {
    return
  }

  it('when awaiting `fetch` using an unknown domain, stack traces are preserved', async () => {
    const browser = await webdriver(next.url, '/api/unknown-domain')

    if (isNextStart) {
      expect(next.cliOutput).toMatch(/at.+\/pages\/api\/unknown-domain.js/)
    } else if (isNextDev) {
      expect(next.cliOutput).toContain('src/fetcher.js')

      expect(await hasRedbox(browser)).toBe(true)
      const source = await getRedboxSource(browser)

      expect(source).toContain('async function anotherFetcher(...args)')
      expect(source).toContain(`fetch(...args)`)

      const description = await getRedboxDescription(browser)
      expect(description).toEqual('TypeError: fetch failed')
    }
  })

  it('when returning `fetch` using an unknown domain, stack traces are preserved', async () => {
    await webdriver(next.url, '/api/unknown-domain-no-await')

    if (process.env.TURBOPACK) {
      // pages_api_unknown-domain-no-await_d8c7f5.js:14:5
      await check(
        () => stripAnsi(next.cliOutput),
        /pages_api_unknown-domain-no-await_.*?\.js/
      )
    } else {
      // webpack-internal:///(middleware)/./pages/api/unknown-domain-no-await.js:10:5
      await check(
        () => stripAnsi(next.cliOutput),
        /at.+\/pages\/api\/unknown-domain-no-await.js/
      )
    }
  })
})
