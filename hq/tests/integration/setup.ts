import { PostgreSqlContainer, type StartedPostgreSqlContainer } from '@testcontainers/postgresql';
import { GenericContainer, type StartedTestContainer, Wait } from 'testcontainers';

let postgresContainer: StartedPostgreSqlContainer | undefined;
let redisContainer: StartedTestContainer | undefined;

export async function setup() {
  console.log('Starting test containers...');

  const [postgres, redis] = await Promise.all([
    new PostgreSqlContainer('postgres:16-alpine')
      .withDatabase('zako3_test')
      .withUsername('test')
      .withPassword('test')
      .start(),
    new GenericContainer('redis:7-alpine')
      .withExposedPorts(6379)
      .withWaitStrategy(Wait.forLogMessage('Ready to accept connections'))
      .start(),
  ]);

  postgresContainer = postgres;
  redisContainer = redis;

  const databaseUrl = postgres.getConnectionUri();
  const redisHost = redis.getHost();
  const redisPort = redis.getMappedPort(6379);
  const redisUrl = `redis://${redisHost}:${redisPort}/0`;

  process.env.DATABASE_URL = databaseUrl;
  process.env.REDIS_URL = redisUrl;
  process.env.NODE_ENV = 'test';
  process.env.LOG_LEVEL = 'error';

  console.log(`PostgreSQL: ${databaseUrl}`);
  console.log(`Redis: ${redisUrl}`);
}

export async function teardown() {
  console.log('Stopping test containers...');

  await Promise.all([
    postgresContainer?.stop(),
    redisContainer?.stop(),
  ]);

  console.log('Test containers stopped');
}
