import Frame from "../../../native/webamp/packages/webamp-modern/src/skin/makiClasses/Frame";

export function installMakiFrames() {
  const setPosition = Frame.prototype.setposition;
  Frame.prototype.setposition = function (position: number) {
    const previous = this._position;
    setPosition.call(this, position);
    // Pane resize callbacks drive ClassicPro's collapse/restore visibility.
    this._invalidateSize();
    if (previous !== this._position) this._uiRoot.vm.dispatch(this, "onsetposition", [
      { type: "INT", value: this._position },
    ]);
  };
  // Wasabi uses left/right to name both panes even for a horizontal divider.
  // Older skins also use top/bottom; both pairs refer to the same two panes.
  Frame.prototype._getEl = function (directions) {
    const self = this as any;
    const ids = directions.map((direction: string, index: number) => self[`_${direction}Id`] || self[index === 0 ? "_leftId" : "_rightId"]);
    const panes = ids.map((id: string) => id ? this.findobject(id) : null);
    if (!panes[0] || !panes[1]) throw new Error(`Frame ${this.getId()} is missing its declared panes: ${ids.join(", ")}`);
    // Frame panes have explicit dimensions; zero means collapsed, not a
    // request to reuse the previous DOM width or bitmap's natural size.
    for (const pane of panes) (pane as any)._allowZeroSize = true;
    return panes;
  };
  const align = Frame.prototype.alignChildren;
  Frame.prototype.alignChildren = function () {
    if ((this as any)._from === "right") {
      const position = Math.max(0, this._position);
      const [first, second] = this._getEl(["left", "right"]);
      first.setXmlAttributes({ x: "0", y: "0", w: String(-position - (position ? 4 : 0)), relatw: "1", h: "0", relath: "1" });
      second.setXmlAttributes({ x: String(-position + (position ? 4 : 0)), relatx: "1", y: "0", w: String(Math.max(0, position - 8)), relatw: "0", h: "0", relath: "1" });
      return;
    }
    if ((this as any)._from !== "top") return align.call(this);
    const position = Math.max(0, this._position);
    const [first, second] = this._getEl(["top", "bottom"]);
    first.setXmlAttributes({ x: "0", y: "0", w: "0", relatw: "1", h: String(Math.max(0, position - 4)), relath: "0" });
    second.setXmlAttributes({ x: "0", y: String(position + 4), w: "0", relatw: "1", h: String(-position - 4), relath: "1" });
  };
}
