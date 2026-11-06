// Minimal history router: the path is the state. Links use `href` + `navigate`.

export const route = $state({ path: location.pathname, query: new URLSearchParams(location.search) });

export function navigate(to: string, replace = false) {
  if (to === location.pathname + location.search) return;
  history[replace ? 'replaceState' : 'pushState'](null, '', to);
  sync();
}

function sync() {
  route.path = location.pathname;
  route.query = new URLSearchParams(location.search);
}

addEventListener('popstate', sync);

/** Click handler for <a href> that keeps the page in the app. */
export function link(e: MouseEvent) {
  const a = (e.currentTarget as HTMLAnchorElement);
  if (e.metaKey || e.ctrlKey || e.shiftKey || e.button !== 0 || a.target) return;
  e.preventDefault();
  navigate(a.getAttribute('href')!);
}

/** Matches "/wedstrijd/:id" style patterns. */
export function match(pattern: string, path = route.path): Record<string, string> | null {
  const p = pattern.split('/');
  const s = path.split('/');
  if (p.length !== s.length) return null;
  const out: Record<string, string> = {};
  for (let i = 0; i < p.length; i++) {
    if (p[i].startsWith(':')) out[p[i].slice(1)] = decodeURIComponent(s[i]);
    else if (p[i] !== s[i]) return null;
  }
  return out;
}
