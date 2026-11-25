// SPDX-License-Identifier: AGPL-3.0-or-later
// Creator token storage. Tokens live only in localStorage, never in URLs.

export const CREATOR_TOKENS_KEY = "trafficlights:creatorTokens";

export interface StorageLike {
  getItem: (key: string) => string | null;
  setItem: (key: string, value: string) => void;
  removeItem: (key: string) => void;
}

export type CreatorTokenMap = Record<string, string>;

function defaultStorage(): StorageLike | null {
  try {
    const candidate: unknown = globalThis.localStorage;
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

export function loadCreatorTokens(storage?: StorageLike): CreatorTokenMap {
  const store = resolveStorage(storage);
  if (store === null) {
    return {};
  }
  try {
    const raw = store.getItem(CREATOR_TOKENS_KEY);
    if (raw === null) {
      return {};
    }
    return parseTokenMap(raw);
  } catch {
    return {};
  }
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
