import type { State } from '../../core';

const urlByState = new WeakMap<State, Map<number, string>>();
const inFlightByState = new WeakMap<State, Set<number>>();

export function setGltfUrl(state: State, entity: number, url: string): void {
  let m = urlByState.get(state);
  if (!m) {
    m = new Map();
    urlByState.set(state, m);
  }
  m.set(entity, url.trim());
}

export function getGltfUrl(state: State, entity: number): string | undefined {
  return urlByState.get(state)?.get(entity);
}

export function isGltfInFlight(state: State, entity: number): boolean {
  return inFlightByState.get(state)?.has(entity) ?? false;
}

export function setGltfInFlight(
  state: State,
  entity: number,
  v: boolean
): void {
  let s = inFlightByState.get(state);
  if (!s) {
    s = new Set();
    inFlightByState.set(state, s);
  }
  if (v) {
    s.add(entity);
  } else {
    s.delete(entity);
  }
}
