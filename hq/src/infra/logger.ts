import pino, { type Logger, type LoggerOptions } from 'pino';

export interface LoggerConfig {
  serviceName: string;
  serviceVersion: string;
  environment: string;
  level?: string;
  otlpEndpoint?: string;
  prettyPrint?: boolean;
}

export function createLogger(config: LoggerConfig): Logger {
  const isProduction = config.environment === 'production';

  const baseOptions: LoggerOptions = {
    level: config.level ?? (isProduction ? 'info' : 'debug'),
    base: {
      service: config.serviceName,
      version: config.serviceVersion,
      env: config.environment,
    },
  };

  if (config.prettyPrint && !isProduction) {
    return pino({
      ...baseOptions,
      transport: {
        target: 'pino-pretty',
        options: {
          colorize: true,
          translateTime: 'SYS:standard',
          ignore: 'pid,hostname',
        },
      },
    });
  }

  if (config.otlpEndpoint) {
    return pino({
      ...baseOptions,
      transport: {
        target: 'pino-opentelemetry-transport',
        options: {
          loggerName: config.serviceName,
          serviceVersion: config.serviceVersion,
          resourceAttributes: {
            'service.name': config.serviceName,
            'service.version': config.serviceVersion,
            'deployment.environment': config.environment,
          },
        },
      },
    });
  }

  return pino(baseOptions);
}

export type { Logger };
