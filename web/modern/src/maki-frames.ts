import Frame from "../../../native/webamp/packages/webamp-modern/src/skin/makiClasses/Frame";

export function installMakiFrames() {
  // Wasabi uses left/right to name both panes even for a horizontal divider.
  // Older skins also use top/bottom; both pairs refer to the same two panes.
  Frame.prototype._getEl = function (directions) {
    const self = this as any;
    const ids = directions.map((direction: string, index: number) => self[`_${direction}Id`] || self[index === 0 ? "_leftId" : "_rightId"]);
    const panes = ids.map((id: string) => id ? this.findobject(id) : null);
    if (!panes[0] || !panes[1]) throw new Error(`Frame ${this.getId()} is missing its declared panes: ${ids.join(", ")}`);
    return panes;
  };
  const align = Frame.prototype.alignChildren;
  Frame.prototype.alignChildren = function () {
    if ((this as any)._from !== "top") return align.call(this);
    const position = Math.max(0, this._position);
    const [first, second] = this._getEl(["top", "bottom"]);
    first.setXmlAttributes({ x: "0", y: "0", w: "0", relatw: "1", h: String(Math.max(0, position - 4)), relath: "0" });
    second.setXmlAttributes({ x: "0", y: String(position + 4), w: "0", relatw: "1", h: String(-position - 4), relath: "1" });
  };
}
