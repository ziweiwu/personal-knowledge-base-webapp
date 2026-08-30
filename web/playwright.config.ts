import { defineConfig, devices } from '@playwright/test';
import { STORAGE_STATE } from './e2e/helpers';

/**
 * The suite drives the real binary serving the real embedded frontend, against copies of
 * the repo's fixtures in a scratch directory. `test/e2e/run-server.sh` makes those copies,
 * so a test that writes can never dirty a fixture the Rust tests depend on being exact.
 */
const PORT = Number(process.env.KBVIEWER_E2E_PORT ?? 4399);

const PROJECTS = [
  { name: 'setup', testMatch: /auth\.setup\.ts/ },
  {
    name: 'desktop',
    use: { ...devices['Desktop Chrome'], storageState: STORAGE_STATE },
    dependencies: ['setup'],
    // The responsive spec asserts drawer and focus-mode behaviour that only exists
    // below the desktop breakpoint.
    testIgnore: [/responsive\.spec\.ts/, /auth\.setup\.ts/],
  },
  {
    name: 'phone-portrait',
    use: { ...devices['iPhone 13'], viewport: { width: 390, height: 844 }, storageState: STORAGE_STATE },
    dependencies: ['setup'],
    testMatch: /(responsive|images)\.spec\.ts/,
  },
  {
    name: 'phone-landscape',
    use: {
      ...devices['iPhone 13 landscape'],
      viewport: { width: 844, height: 390 },
      storageState: STORAGE_STATE,
    },
    dependencies: ['setup'],
    testMatch: /(responsive|images)\.spec\.ts/,
  },
];

export default defineConfig({
  testDir: './e2e',
  fullyParallel: false, // one server, one account, and several specs write files
  workers: 1,
  forbidOnly: Boolean(process.env.CI),
  retries: process.env.CI ? 1 : 0,
  // The HTML report is what the CI job uploads when something fails; without it the
  // artifact step would have nothing to collect.
  reporter: process.env.CI ? [['github'], ['list'], ['html', { open: 'never' }]] : [['list']],
  use: {
    baseURL: `http://127.0.0.1:${PORT}`,
    trace: 'retain-on-failure',
    screenshot: 'only-on-failure',
  },
  projects: PROJECTS,
  webServer: {
    command: 'bash ../test/e2e/run-server.sh',
    url: `http://127.0.0.1:${PORT}/`,
    reuseExistingServer: !process.env.CI,
    timeout: 180_000,
    stdout: 'pipe',
    stderr: 'pipe',
  },
});
