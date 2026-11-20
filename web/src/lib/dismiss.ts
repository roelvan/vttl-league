/**
 * A press outside an open popover closes it (light dismiss of the browser), but the click
 * then also goes to the element below. This stops that click: the press only closes.
 * A press on a button that opens a widget (`aria-haspopup="dialog"`) goes through, so the
 * user can go from one player or team to the next with one click.
 *
 * Call it while the popover is open. Returns the function that stops the watch; a click
 * that is already on its way is still stopped after that.
 */
export function swallowOutsideClick(el: HTMLElement): () => void {
  const down = (e: PointerEvent) => {
    const t = e.target;
    if (!(t instanceof Element) || el.contains(t) || t.closest('[aria-haspopup="dialog"]')) return;
    const eat = (c: MouseEvent) => {
      c.preventDefault();
      c.stopPropagation();
      stop();
    };
    // No click came (a drag, a scroll): the next press or key is a new action.
    const stop = () => {
      window.removeEventListener('click', eat, true);
      window.removeEventListener('pointerdown', stop, true);
      window.removeEventListener('keydown', stop, true);
    };
    window.addEventListener('click', eat, true);
    window.addEventListener('pointerdown', stop, true);
    window.addEventListener('keydown', stop, true);
  };
  window.addEventListener('pointerdown', down, true);
  return () => window.removeEventListener('pointerdown', down, true);
}
