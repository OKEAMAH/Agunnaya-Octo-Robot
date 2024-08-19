import { mkdir, writeFile } from 'fs/promises'
import { join } from 'path'
import {
  run,
  useTempDir,
  projectFilesShouldExist,
  projectFilesShouldNotExist,
} from './utils'
import { createNextInstall } from '../../lib/create-next-install'
import { trace } from 'next/dist/trace'

let nextInstall: Awaited<ReturnType<typeof createNextInstall>>
beforeAll(async () => {
  nextInstall = await createNextInstall({
    parentSpan: trace('test'),
    keepRepoDir: Boolean(process.env.NEXT_TEST_SKIP_CLEANUP),
  })
})

describe.skip('create-next-app', () => {
  it('should not create if the target directory is not empty', async () => {
    await useTempDir(async (cwd) => {
      const projectName = 'non-empty-dir'
      await mkdir(join(cwd, projectName))
      const pkg = join(cwd, projectName, 'package.json')
      await writeFile(pkg, `{ "name": "${projectName}" }`)

      const res = await run(
        [
          projectName,
          '--ts',
          '--app',
          '--no-turbo',
          '--no-eslint',
          '--no-tailwind',
          '--no-src-dir',
          '--no-import-alias',
        ],
        nextInstall.installDir,
        {
          cwd,
          reject: false,
        }
      )
      expect(res.exitCode).toBe(1)
      expect(res.stdout).toMatch(/contains files that could conflict/)
    })
  })

  it('should not create if the target directory is not writable', async () => {
    const expectedErrorMessage =
      /you do not have write permissions for this folder|EPERM: operation not permitted/

    await useTempDir(async (cwd) => {
      const projectName = 'dir-not-writable'

      // if the folder isn't able to be write restricted we can't test so skip
      if (
        await writeFile(join(cwd, 'test'), 'hello')
          .then(() => true)
          .catch(() => false)
      ) {
        console.warn(
          `Test folder is not write restricted skipping write permission test`
        )
        return
      }

      const res = await run(
        [
          projectName,
          '--ts',
          '--app',
          '--eslint',
          '--no-tailwind',
          '--no-src-dir',
          '--no-import-alias',
        ],
        nextInstall.installDir,
        {
          cwd,
          reject: false,
        }
      )

      expect(res.stderr).toMatch(expectedErrorMessage)
      expect(res.exitCode).toBe(1)
    }, 0o500).catch((err) => {
      if (!expectedErrorMessage.test(err.message)) {
        throw err
      }
    })
  })
  it('should not install dependencies if --skip-install', async () => {
    await useTempDir(async (cwd) => {
      const projectName = 'empty-dir'

      const res = await run(
        [
          projectName,
          '--ts',
          '--app',
          '--no-turbo',
          '--no-eslint',
          '--no-tailwind',
          '--no-src-dir',
          '--no-import-alias',
          '--skip-install',
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
        files: ['.gitignore', 'package.json'],
      })
      projectFilesShouldNotExist({ cwd, projectName, files: ['node_modules'] })
    })
  })
})
