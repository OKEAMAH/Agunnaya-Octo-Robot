import { nextTestSetup } from 'e2e-utils'

describe('app dir - css with pageextensions', () => {
  const { next, skipped } = nextTestSetup({
    files: __dirname,
    skipDeployment: true,
    dependencies: {
      '@picocss/pico': '1.5.7',
      react: '19.0.0-beta-04b058868c-20240508',
      'react-dom': '19.0.0-beta-04b058868c-20240508',
      sass: 'latest',
    },
  })

  if (skipped) {
    return
  }

  describe('css support with pageextensions', () => {
    describe('page in app directory with pageextention, css should work', () => {
      it('should support global css inside layout', async () => {
        const browser = await next.browser('/css-pageextensions')
        expect(
          await browser.eval(
            `window.getComputedStyle(document.querySelector('h1')).color`
          )
        ).toBe('rgb(255, 0, 0)')
      })
    })
  })
})
