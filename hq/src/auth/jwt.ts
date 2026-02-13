import { createRemoteJWKSet, jwtVerify, type JWTVerifyResult } from 'jose';
import { ok, err, type Result } from 'zako3-settings';
import { type JWTPayload, AuthErrors } from './context.js';

export interface JWTVerifierConfig {
  jwksUrl: string;
  issuer?: string;
  audience?: string;
}

export interface IJWTVerifier {
  verify(token: string): Promise<Result<JWTPayload>>;
}

export function createJWTVerifier(config: JWTVerifierConfig): IJWTVerifier {
  const jwks = createRemoteJWKSet(new URL(config.jwksUrl));

  return {
    async verify(token: string): Promise<Result<JWTPayload>> {
      try {
        const result: JWTVerifyResult = await jwtVerify(token, jwks, {
          issuer: config.issuer,
          audience: config.audience,
        });

        const payload = result.payload;

        if (!payload.sub || typeof payload.sub !== 'string') {
          return err(AuthErrors.INVALID_TOKEN);
        }

        return ok({
          sub: payload.sub,
          iat: payload.iat ?? Math.floor(Date.now() / 1000),
          exp: payload.exp ?? Math.floor(Date.now() / 1000) + 3600,
        });
      } catch (error) {
        if (error instanceof Error) {
          if (error.message.includes('expired')) {
            return err(AuthErrors.TOKEN_EXPIRED);
          }
          if (error.message.includes('fetch')) {
            return err(AuthErrors.JWKS_FETCH_FAILED);
          }
        }
        return err(AuthErrors.INVALID_TOKEN);
      }
    },
  };
}

export interface StaticJWTVerifierConfig {
  publicKey: string;
  algorithm: 'RS256' | 'ES256';
  issuer?: string;
  audience?: string;
}

export function createStaticJWTVerifier(
  config: StaticJWTVerifierConfig
): IJWTVerifier {
  let keyPromise: Promise<CryptoKey> | undefined;

  async function getKey(): Promise<CryptoKey> {
    if (!keyPromise) {
      keyPromise = (async () => {
        const pemContents = config.publicKey
          .replace(/-----BEGIN PUBLIC KEY-----/, '')
          .replace(/-----END PUBLIC KEY-----/, '')
          .replace(/\s/g, '');

        const binaryKey = Uint8Array.from(atob(pemContents), (c) =>
          c.charCodeAt(0)
        );

        const algorithm =
          config.algorithm === 'RS256'
            ? { name: 'RSASSA-PKCS1-v1_5', hash: 'SHA-256' }
            : { name: 'ECDSA', namedCurve: 'P-256' };

        return crypto.subtle.importKey('spki', binaryKey, algorithm, true, [
          'verify',
        ]);
      })();
    }
    return keyPromise;
  }

  return {
    async verify(token: string): Promise<Result<JWTPayload>> {
      try {
        const key = await getKey();
        const result = await jwtVerify(token, key, {
          issuer: config.issuer,
          audience: config.audience,
        });

        const payload = result.payload;

        if (!payload.sub || typeof payload.sub !== 'string') {
          return err(AuthErrors.INVALID_TOKEN);
        }

        return ok({
          sub: payload.sub,
          iat: payload.iat ?? Math.floor(Date.now() / 1000),
          exp: payload.exp ?? Math.floor(Date.now() / 1000) + 3600,
        });
      } catch (error) {
        if (error instanceof Error && error.message.includes('expired')) {
          return err(AuthErrors.TOKEN_EXPIRED);
        }
        return err(AuthErrors.INVALID_TOKEN);
      }
    },
  };
}
