// Function body shared by mouse, long-press and reorder-handle gestures.
// Return the source queue index at the visible gap, including filtered rows.
const pane = document.getElementById("playlist-rows");
if (!pane) return null;
const tracks = [...pane.querySelectorAll(".track:not(.row-drag-anchor)")];
tracks.forEach(row => row.classList.remove("reorder-above", "reorder-below"));
const bounds = pane.getBoundingClientRect();
const header = pane.querySelector(".columns")?.getBoundingClientRect();
if (x < bounds.left || x >= bounds.left + pane.clientWidth
    || y < Math.max(bounds.top, header?.bottom || bounds.top)
    || y >= bounds.top + pane.clientHeight || !tracks.length) return null;
for (const row of tracks) {
    const rect = row.getBoundingClientRect();
    if (y < rect.top + rect.height / 2) {
        row.classList.add("reorder-above");
        return Number(row.dataset.index);
    }
}
tracks[tracks.length - 1].classList.add("reorder-below");
return queueLength;
