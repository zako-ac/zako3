import { defineConfig } from 'vitest/config';

export default defineConfig({
  test: {
    include: ['tests/integration/**/*.test.ts'],
    globalSetup: './tests/integration/setup.ts',
    testTimeout: 30000,
    hookTimeout: 120000,
    // Run tests sequentially to avoid race conditions with migrations
    fileParallelism: false,
    sequence: {
      shuffle: false,
    },
  },
});
