// A single pointer gesture. Keep the model unchanged until a valid drop.
const button = event.target.closest('.playlist-tab > button[role="tab"]');
if (!button || event.button !== 0 || document.querySelector('.workspace-close')) return;
const source = button.parentElement;
const strip = source.parentElement;
const key = source.dataset.tabKey;
const abort = new AbortController();
const options = {capture: true, signal: abort.signal};
let x = event.clientX, y = event.clientY, dragging = false, before, frame, hold;
const startX = x, startY = y;
const marker = document.createElement('div');
marker.className = 'playlist-tab-insertion';
function update() {
    const rect = strip.getBoundingClientRect();
    before = undefined;
    marker.hidden = true;
    if (x < rect.left || x > rect.right || y < rect.top || y > rect.bottom) return;
    const tabs = [...strip.querySelectorAll('.playlist-tab')].filter(tab => tab !== source);
    const next = tabs.find(tab => x < tab.getBoundingClientRect().left + tab.offsetWidth / 2);
    before = next ? next.dataset.tabKey : null;
    const edge = next ? next.getBoundingClientRect().left : (tabs.at(-1)?.getBoundingClientRect().right ?? rect.left);
    Object.assign(marker.style, {left: `${Math.max(rect.left, Math.min(rect.right - 3, edge))}px`, top: `${rect.top + 2}px`, height: `${rect.height - 4}px`});
    marker.hidden = false;
}
function tick() {
    if (!dragging) return;
    const rect = strip.getBoundingClientRect();
    if (y >= rect.top && y <= rect.bottom)
        strip.scrollLeft += x < rect.left + 28 ? -8 : x > rect.right - 28 ? 8 : 0;
    update(); frame = requestAnimationFrame(tick);
}
function start() {
    dragging = true;
    source.classList.add('tab-dragging');
    document.body.append(marker);
    strip.setPointerCapture(event.pointerId);
    tick();
}
function finish(commit) {
    clearTimeout(hold); cancelAnimationFrame(frame);
    abort.abort(); marker.remove(); source.classList.remove('tab-dragging');
    if (strip.hasPointerCapture(event.pointerId)) strip.releasePointerCapture(event.pointerId);
    if (dragging) {
        // Suppress the synthetic click that follows pointerup, including touch.
        const block = e => { e.preventDefault(); e.stopImmediatePropagation(); };
        document.addEventListener('click', block, {capture:true, once:true});
        setTimeout(() => document.removeEventListener('click', block, true), 0);
        if (commit && before !== undefined && source.isConnected) move(key, before);
    }
}
if (event.pointerType !== 'mouse') hold = setTimeout(start, 350);
document.addEventListener('pointermove', e => {
    if (e.pointerId !== event.pointerId) return;
    x = e.clientX; y = e.clientY;
    const distance = Math.hypot(x - startX, y - startY);
    if (!dragging && distance > 7) {
        if (event.pointerType === 'mouse') start();
        else { finish(false); return; } // A swipe still scrolls the tab strip.
    }
    if (dragging) { e.preventDefault(); e.stopPropagation(); update(); }
}, options);
document.addEventListener('pointerup', e => {
    if (e.pointerId !== event.pointerId) return;
    x = e.clientX; y = e.clientY;
    if (dragging) { update(); e.preventDefault(); e.stopPropagation(); }
    finish(true);
}, options);
document.addEventListener('touchmove', e => { if (dragging) e.preventDefault(); }, {capture:true, passive:false, signal:abort.signal});
document.addEventListener('pointercancel', () => finish(false), options);
document.addEventListener('keydown', e => { if (e.key === 'Escape') finish(false); }, options);
window.addEventListener('blur', () => finish(false), {signal:abort.signal});
strip.addEventListener('contextmenu', e => { if (dragging) e.preventDefault(); }, options);
