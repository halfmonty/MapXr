/**
 * Structured logger wrapper.
 *
 * `logger.error` always emits. `logger.info` and `logger.warn` are gated on
 * the dev build so they never appear in production bundles.
 */

/* eslint-disable no-console */
const isDev = import.meta.env.DEV;

export const logger = {
  info(msg: string, ...args: unknown[]): void {
    if (isDev) console.info(`[mapxr] ${msg}`, ...args);
  },
  warn(msg: string, ...args: unknown[]): void {
    if (isDev) console.warn(`[mapxr] ${msg}`, ...args);
  },
  error(msg: string, ...args: unknown[]): void {
    console.error(`[mapxr] ${msg}`, ...args);
  },
};
