// SPDX-License-Identifier: AGPL-3.0-or-later
// Browser storage: creator tokens and viewer names live in localStorage,
// never in URLs; the viewer reconnect key lives in sessionStorage (per tab).

export const CREATOR_TOKENS_KEY = "trafficlights:creatorTokens";
export const VIEWER_NAMES_KEY = "trafficlights:viewerNames";
export const LAST_VIEWER_NAME_KEY = "trafficlights:lastViewerName";
export const VIEWER_KEY_KEY = "trafficlights:viewerKey";

const VIEWER_KEY_BYTES = 16;
const HEX_RADIX = 16;
const HEX_BYTE_DIGITS = 2;

export interface StorageLike {
  getItem: (key: string) => string | null;
  setItem: (key: string, value: string) => void;
  removeItem: (key: string) => void;
}

export type CreatorTokenMap = Record<string, string>;

type StorageKind = "localStorage" | "sessionStorage";

function defaultStorage(kind: StorageKind = "localStorage"): StorageLike | null {
  try {
    const candidate: unknown = globalThis[kind];
    if (candidate === null || candidate === undefined) {
      return null;
    }
    return assertStorage(candidate);
  } catch {
    return null;
  }
}

function assertStorage(candidate: unknown): StorageLike | null {
  if (typeof candidate !== "object" || candidate === null) {
    return null;
  }
  const getItem: unknown = Reflect.get(candidate, "getItem");
  const setItem: unknown = Reflect.get(candidate, "setItem");
  const removeItem: unknown = Reflect.get(candidate, "removeItem");
  if (
    typeof getItem !== "function" ||
    typeof setItem !== "function" ||
    typeof removeItem !== "function"
  ) {
    return null;
  }
  return {
    getItem: (key: string): string | null => {
      const value: unknown = Reflect.apply(getItem, candidate, [key]);
      return typeof value === "string" ? value : null;
    },
    setItem: (key: string, value: string): void => {
      Reflect.apply(setItem, candidate, [key, value]);
    },
    removeItem: (key: string): void => {
      Reflect.apply(removeItem, candidate, [key]);
    },
  };
}

function resolveStorage(storage?: StorageLike): StorageLike | null {
  return storage ?? defaultStorage();
}

function readMap(store: StorageLike | null, key: string): CreatorTokenMap {
  if (store === null) {
    return {};
  }
  try {
    const raw = store.getItem(key);
    return raw === null ? {} : parseTokenMap(raw);
  } catch {
    return {};
  }
}

function readText(store: StorageLike | null, key: string): string | null {
  if (store === null) {
    return null;
  }
  try {
    const value = store.getItem(key);
    return value !== null && value.length > 0 ? value : null;
  } catch {
    return null;
  }
}

function writeText(store: StorageLike | null, key: string, value: string): void {
  if (store === null) {
    return;
  }
  try {
    store.setItem(key, value);
  } catch {
    // Storage may be full or blocked; the value then lasts for this page only.
  }
}

export function loadCreatorTokens(storage?: StorageLike): CreatorTokenMap {
  return readMap(resolveStorage(storage), CREATOR_TOKENS_KEY);
}

function parseTokenMap(raw: string): CreatorTokenMap {
  try {
    const parsed: unknown = JSON.parse(raw);
    if (typeof parsed !== "object" || parsed === null || Array.isArray(parsed)) {
      return {};
    }
    const out: CreatorTokenMap = {};
    for (const [key, value] of Object.entries(parsed)) {
      if (typeof value === "string" && value.length > 0) {
        out[key] = value;
      }
    }
    return out;
  } catch {
    return {};
  }
}

export function getCreatorToken(sessionId: string, storage?: StorageLike): string | null {
  const tokens = loadCreatorTokens(storage);
  const token = tokens[sessionId];
  return typeof token === "string" && token.length > 0 ? token : null;
}

export function hasCreatorToken(sessionId: string, storage?: StorageLike): boolean {
  return getCreatorToken(sessionId, storage) !== null;
}

export function saveCreatorToken(sessionId: string, token: string, storage?: StorageLike): void {
  const store = resolveStorage(storage);
  if (store === null) {
    return;
  }
  try {
    const tokens = loadCreatorTokens(store);
    tokens[sessionId] = token;
    store.setItem(CREATOR_TOKENS_KEY, JSON.stringify(tokens));
  } catch {
    // Storage may be unavailable in private mode; creation still succeeds
    // for this tab via navigation state in the caller.
  }
}

export function removeCreatorToken(sessionId: string, storage?: StorageLike): void {
  const store = resolveStorage(storage);
  if (store === null) {
    return;
  }
  try {
    const tokens = loadCreatorTokens(store);
    delete tokens[sessionId];
    store.setItem(CREATOR_TOKENS_KEY, JSON.stringify(tokens));
  } catch {
    // Ignore unavailable storage.
  }
}

/** The name this browser gave in session `sessionId`, if any. */
export function getViewerName(sessionId: string, storage?: StorageLike): string | null {
  const name = readMap(resolveStorage(storage), VIEWER_NAMES_KEY)[sessionId];
  return typeof name === "string" && name.length > 0 ? name : null;
}

/** Remember `name` for session `sessionId`, and as the name to suggest next time. */
export function saveViewerName(sessionId: string, name: string, storage?: StorageLike): void {
  const store = resolveStorage(storage);
  const names = readMap(store, VIEWER_NAMES_KEY);
  names[sessionId] = name;
  writeText(store, VIEWER_NAMES_KEY, JSON.stringify(names));
  writeText(store, LAST_VIEWER_NAME_KEY, name);
}

/** The name this browser gave most recently in any session. */
export function getLastViewerName(storage?: StorageLike): string | null {
  return readText(resolveStorage(storage), LAST_VIEWER_NAME_KEY);
}

let memoryViewerKey: string | null = null;

function randomKey(): string | null {
  const cryptoApi: Crypto | undefined = globalThis.crypto;
  if (cryptoApi === undefined) {
    return null;
  }
  const bytes = cryptoApi.getRandomValues(new Uint8Array(VIEWER_KEY_BYTES));
  return Array.from(bytes, (byte) => byte.toString(HEX_RADIX).padStart(HEX_BYTE_DIGITS, "0")).join(
    "",
  );
}

/**
 * This tab's random reconnect key, sent with `identify`: a reconnect from
 * the same tab takes its name back from its own stale connection, while
 * another tab or browser cannot take a name that is in use.
 */
export function getViewerKey(storage?: StorageLike): string | null {
  const store = storage ?? defaultStorage("sessionStorage");
  const stored = readText(store, VIEWER_KEY_KEY) ?? memoryViewerKey;
  if (stored !== null) {
    return stored;
  }
  const key = randomKey();
  if (key !== null) {
    memoryViewerKey = key;
    writeText(store, VIEWER_KEY_KEY, key);
  }
  return key;
}
