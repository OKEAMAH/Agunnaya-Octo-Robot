import { join } from 'path'
import { nextTestSetup } from 'e2e-utils'
import { hasRedbox } from 'next-test-utils'

describe('optimizePackageImports - mui', () => {
  const { next } = nextTestSetup({
    env: {
      NEXT_TEST_MODE: '1',
    },
    files: join(__dirname, 'fixture-tremor'),

    dependencies: {
      '@headlessui/react': '^1.7.18',
      '@headlessui/tailwindcss': '^0.2.0',
      '@remixicon/react': '^4.2.0',
      '@tremor/react': '^3.14.1',
    },
  })

  it('should work', async () => {
    // Without barrel optimization, the reproduction breaks.
    const browser = await next.browser('/')
    expect(await hasRedbox(browser)).toBe(false)
  })
})
