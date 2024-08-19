# Testing

## Running tests

Before you start to run tests, you need to [build the project first](./building.md):

```bash
pnpm build
```

We recommend running the tests in a specific directory pattern.

For example, running one test in the production test suite:

Running tests in the `test/e2e/app-dir/app` test suite in production mode (`next build` and `next start`):

```sh
pnpm test-start test/e2e/app-dir/app/
```

Running tests in the `test/e2e/app-dir/app` test suite in development mode (`next dev`):

```sh
pnpm test-dev test/e2e/app-dir/app/
```

When the test runs it will open the browser that is in the background by default, you won't see the browser window.

When you want to debug a particular test you can replace `pnpm test-start` with `pnpm testonly-start` to see the browser window open.

```sh
pnpm testonly-start test/e2e/app-dir/app/
```

**End-to-end (e2e)** tests are run in complete isolation from the repository.
When you run an `test/e2e`, `test/production`, or `test/development` tests, a local version of Next.js will be created inside your system's temp folder (eg. /tmp),
which is then linked to an isolated version of the application. A server is started on a random port, against which the tests will run.
After all tests have finished, the server is destroyed and all remaining files are deleted from the temp folder. All of this logic is handle by `createNextDescribe` automatically.

## Writing tests for Next.js

### Getting Started

You can set up a new test using `pnpm new-test` which will start from a template related to the test type.

### Test Types in Next.js

- e2e: Runs against `next dev`, `next start`, and deployed to Vercel.
- development: Runs against `next dev`.
- production: Runs against `next start`.
- integration: Historical location of tests. Runs misc checks and modes. Ideally, we don't add new test suites here anymore as these tests are not isolated from the monorepo.
- unit: Very fast tests that should run without a browser or run `next` and should be testing a specific utility.

For the e2e, development, and production tests the `createNextDescribe` utility should be used. An example is available [here](../../test/e2e/example.txt). This creates an isolated Next.js install to ensure nothing in the monorepo is relied on accidentally causing incorrect tests. `pnpm next-test` automatically uses `createNextDescribe`

All new test suites should be written in TypeScript either `.ts` (or `.tsx` for unit tests). This will help ensure we catch smaller issues in tests that could cause flakey or incorrect tests.

If a test suite already exists that relates closely to the item being tested (e.g. hash navigation relates to existing navigation test suites) the new checks can be added to the existing test suite.

### Best Practices

- When checking for a condition that might take time, ensure it is waited for either using the browser `waitForElement` or using the `check` util in `next-test-utils`.
- When applying a fix, ensure the test fails without the fix. This makes sure the test will properly catch regressions.

### Helpful environment variables

Some test-specific environment variables can be used to help debug isolated tests better, these can be leveraged by prefixing the `pnpm test` command.

- When investigating failures in isolated tests you can use `NEXT_TEST_SKIP_CLEANUP=1` to prevent deleting the temp folder created for the test, then you can run `pnpm next` while inside of the temp folder to debug the fully set-up test project.
- You can also use `NEXT_SKIP_ISOLATE=1` if the test doesn't need to be installed to debug and it will run inside of the Next.js repo instead of the temp directory, this can also reduce test times locally but is not compatible with all tests.
- The `NEXT_TEST_MODE` env variable allows toggling specific test modes for the `e2e` folder, it can be used when not using `pnpm test-dev` or `pnpm test-start` directly. Valid test modes can be seen here: https://github.com/vercel/next.js/blob/aa664868c102ddc5adc618415162d124503ad12e/test/lib/e2e-utils.ts#L46
- You can use `NEXT_TEST_PREFER_OFFLINE=1` while testing to configure the package manager to include the [`--prefer-offline`](https://pnpm.io/cli/install#--prefer-offline) argument during test setup. This is helpful when running tests in internet restricted environments such as planes or public wifi.

### Debugging

When tests are run in CI and a test failure occurs we attempt to capture traces of the playwright run to make debugging the failure easier. A test-trace artifact should be uploaded after the workflow completes which can be downloaded, unzipped, and then inspected with `pnpm playwright show-trace ./path/to/trace`

### Profiling tests

Add `NEXT_TEST_TRACE=1` to enable test profiling. It's useful for improving our testing infrastructure.

### Recording the browser using Replay.io

Using [Replay.io](https://www.replay.io/) you can record amd time-travel debug the browser.

1. Clear all local replays using `pnpm replay rm-all`
1. Run the test locally using the `RECORD_REPLAY=1` environment variables. I.e. `RECORD_REPLAY=1 pnpm test-dev test/e2e/app-dir/app/index.test.ts`
1. Upload all the replays to your workspace using your the API key: `RECORD_REPLAY_API_KEY=addkeyhere pnpm replay upload-all`
1. Check the uploaded replays in your workspace, while uploading it provides the URLs.

### Testing Turbopack

To run the test suite using Turbopack you can use the `TURBOPACK=1` environment variable:

```sh
TURBOPACK=1 pnpm test-dev test/e2e/app-dir/app/
```
