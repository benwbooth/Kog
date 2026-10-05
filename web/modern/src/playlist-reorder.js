const mimeType = "text/x-kog-playlist";

export function clearPlaylistDropMarker(pane) {
  for (const row of pane.querySelectorAll("[data-index]"))
    row.classList.remove("kog-reorder-before", "kog-reorder-after");
}

export function playlistDropTarget(pane, x, y) {
  clearPlaylistDropMarker(pane);
  const bounds = pane.getBoundingClientRect();
  if (x < bounds.left || x >= bounds.left + pane.clientWidth
      || y < bounds.top || y >= bounds.top + pane.clientHeight) return null;
  const rows = [...pane.querySelectorAll("[data-index]")];
  for (const row of rows) {
    const rect = row.getBoundingClientRect();
    if (y < rect.top + rect.height / 2) {
      row.classList.add("kog-reorder-before");
      return Number(row.dataset.index);
    }
  }
  rows.at(-1)?.classList.add("kog-reorder-after");
  return rows.length;
}

export function bindPlaylistReorder(pane, move) {
  const accepts = event => Array.from(event.dataTransfer?.types || []).includes(mimeType);
  pane.addEventListener("dragover", event => {
    if (!accepts(event)) return;
    const target = playlistDropTarget(pane, event.clientX, event.clientY);
    if (target === null) return;
    event.preventDefault();
    event.dataTransfer.dropEffect = "move";
  });
  pane.addEventListener("drop", event => {
    if (!accepts(event)) return;
    const target = playlistDropTarget(pane, event.clientX, event.clientY);
    clearPlaylistDropMarker(pane);
    if (target === null) return;
    event.preventDefault();
    event.stopPropagation();
    move(target);
  });
  pane.addEventListener("dragleave", event => {
    if (!event.relatedTarget || !pane.contains(event.relatedTarget)) clearPlaylistDropMarker(pane);
  });
  pane.addEventListener("dragend", () => clearPlaylistDropMarker(pane));
}
