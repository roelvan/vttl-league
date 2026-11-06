// A change to a sheet: set the value at a path. Same rules as src/sheets.rs `apply`.

export type Path = (string | number)[];
export interface Op {
  p: Path;
  v: unknown;
}

/** Applies ops to a copy-on-write clone of `doc` and returns the new document. */
export function applyOps<T>(doc: T, ops: Op[]): T {
  if (!ops.length) return doc;
  const root = structuredClone(doc) as unknown;
  for (const op of ops) setAt(root, op.p, op.v);
  return root as T;
}

/** Sets in place. Missing objects/arrays are created; arrays grow with nulls. */
export function setAt(root: unknown, path: Path, value: unknown) {
  let cur = root as Record<string | number, unknown>;
  for (let i = 0; i < path.length - 1; i++) {
    const k = path[i];
    let next = cur[k];
    if (next === null || typeof next !== 'object') {
      next = typeof path[i + 1] === 'number' ? [] : {};
      cur[k] = next;
    }
    cur = next as Record<string | number, unknown>;
  }
  const last = path[path.length - 1];
  if (Array.isArray(cur) && typeof last === 'number') while (cur.length < last) cur.push(null);
  cur[last] = value;
}

export function getAt(root: unknown, path: Path): unknown {
  let cur = root as Record<string | number, unknown> | undefined;
  for (const k of path) {
    if (cur === null || typeof cur !== 'object') return undefined;
    cur = cur[k] as Record<string | number, unknown> | undefined;
  }
  return cur;
}
