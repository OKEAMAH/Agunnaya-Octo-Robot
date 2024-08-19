/* eslint-env jest */
import { nextTestSetup, isNextDev } from 'e2e-utils'
import { getRedboxSource, hasRedbox, retry } from 'next-test-utils'
import * as fs from 'fs'
import * as path from 'path'
import * as os from 'os'
import * as Log from './utils/log'

// using unstable_after is a compile-time error in build mode.
const _describe = isNextDev ? describe : describe.skip

_describe('unstable_after() - pages', () => {
  const logFileDir = fs.mkdtempSync(path.join(os.tmpdir(), 'logs-'))
  const logFile = path.join(logFileDir, 'logs.jsonl')

  const { next } = nextTestSetup({
    files: __dirname,
    env: {
      PERSISTENT_LOG_FILE: logFile,
    },
  })

  let currentCliOutputIndex = 0
  beforeEach(() => {
    currentCliOutputIndex = next.cliOutput.length
  })

  const getLogs = () => {
    return Log.readCliLogs(next.cliOutput.slice(currentCliOutputIndex))
  }

  it('runs in middleware', async () => {
    const requestId = `${Date.now()}`
    const res = await next.fetch(
      `/middleware/redirect-source?requestId=${requestId}`,
      {
        redirect: 'follow',
        headers: {
          cookie: 'testCookie=testValue',
        },
      }
    )

    expect(res.status).toBe(200)
    await retry(() => {
      expect(getLogs()).toContainEqual({
        source: '[middleware] /middleware/redirect-source',
        requestId,
        cookies: { testCookie: 'testValue' },
      })
    })
  })

  describe('invalid usages', () => {
    describe('errors at compile time when used in pages dir', () => {
      it.each([
        {
          title: 'errors when used in getServerSideProps',
          path: '/pages-dir/invalid-in-gssp',
        },
        {
          title: 'errors when used in getStaticProps',
          path: '/pages-dir/123/invalid-in-gsp',
        },
        {
          title: 'errors when used in within a page component',
          path: '/pages-dir/invalid-in-page',
        },
      ])('$title', async ({ path }) => {
        const browser = await next.browser(path)

        expect(await hasRedbox(browser)).toBe(true)
        expect(await getRedboxSource(browser)).toMatch(
          /You're importing a component that needs "?unstable_after"?\. That only works in a Server Component which is not supported in the pages\/ directory\./
        )
        expect(getLogs()).toHaveLength(0)
      })
    })
  })
})
