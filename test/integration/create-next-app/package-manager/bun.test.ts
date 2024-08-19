import { trace } from 'next/dist/trace'
import { createNextInstall } from '../../../lib/create-next-install'
import {
  command,
  DEFAULT_FILES,
  FULL_EXAMPLE_PATH,
  projectFilesShouldExist,
  run,
  useTempDir,
} from '../utils'

const lockFile = 'bun.lockb'
const files = [...DEFAULT_FILES, lockFile]

beforeEach(async () => {
  await command('bun', ['--version'])
    // install bun if not available
    .catch(() => command('npm', ['i', '-g', 'bun']))
})

let nextInstall: Awaited<ReturnType<typeof createNextInstall>>
beforeAll(async () => {
  nextInstall = await createNextInstall({
    parentSpan: trace('test'),
    keepRepoDir: Boolean(process.env.NEXT_TEST_SKIP_CLEANUP),
  })
})

describe.skip('create-next-app with package manager bun', () => {
  it('should use bun for --use-bun flag', async () => {
    await useTempDir(async (cwd) => {
      const projectName = 'use-bun'
      const res = await run(
        [
          projectName,
          '--ts',
          '--app',
          '--use-bun',
          '--no-turbo',
          '--no-eslint',
          '--no-src-dir',
          '--no-tailwind',
          '--no-import-alias',
        ],
        nextInstall.installDir,
        {
          cwd,
        }
      )

      expect(res.exitCode).toBe(0)
      projectFilesShouldExist({
        cwd,
        projectName,
        files,
      })
    })
  })

  it('should use bun when user-agent is bun', async () => {
    await useTempDir(async (cwd) => {
      const projectName = 'user-agent-bun'
      const res = await run(
        [
          projectName,
          '--ts',
          '--app',
          '--no-turbo',
          '--no-eslint',
          '--no-src-dir',
          '--no-tailwind',
          '--no-import-alias',
        ],
        nextInstall.installDir,
        {
          cwd,
          env: { npm_config_user_agent: 'bun' },
        }
      )

      expect(res.exitCode).toBe(0)
      projectFilesShouldExist({
        cwd,
        projectName,
        files,
      })
    })
  })

  it('should use bun for --use-bun flag with example', async () => {
    await useTempDir(async (cwd) => {
      const projectName = 'use-bun-with-example'
      const res = await run(
        [projectName, '--use-bun', '--example', FULL_EXAMPLE_PATH],
        nextInstall.installDir,
        { cwd }
      )

      expect(res.exitCode).toBe(0)
      projectFilesShouldExist({
        cwd,
        projectName,
        files,
      })
    })
  })

  it('should use bun when user-agent is bun with example', async () => {
    await useTempDir(async (cwd) => {
      const projectName = 'user-agent-bun-with-example'
      const res = await run(
        [projectName, '--example', FULL_EXAMPLE_PATH],
        nextInstall.installDir,
        {
          cwd,
          env: { npm_config_user_agent: 'bun' },
        }
      )

      expect(res.exitCode).toBe(0)
      projectFilesShouldExist({
        cwd,
        projectName,
        files,
      })
    })
  })
})
