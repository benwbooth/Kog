import GuiObj from "../../../native/webamp/packages/webamp-modern/src/skin/makiClasses/GuiObj";
import ConfigAttribute from "../../../native/webamp/packages/webamp-modern/src/skin/makiClasses/ConfigAttribute";
import Slider from "../../../native/webamp/packages/webamp-modern/src/skin/makiClasses/Slider";

const bindings = new WeakMap<object, { attribute: ConfigAttribute; listener: () => void }>();

export function installMakiConfigBindings() {
  Slider.prototype._cfgAttribChanged = function (value: string) {
    // PSliderWnd::onReloadConfig uses setPosition(value, 0): update the thumb
    // without generating another user action or writing the setting back.
    const position = Number.parseInt(value, 10) || 0;
    this._position = this._high ? Math.max(0, Math.min(1, position / this._high)) : 0;
    this._renderThumbPosition();
  };
  GuiObj.prototype._setConfigAttrib = function (specification: string) {
    const previous = bindings.get(this);
    if (previous) previous.attribute.off("datachanged", previous.listener);
    bindings.delete(this);
    this._configAttrib = null;
    const separator = specification.indexOf(";");
    if (separator < 0) return;
    const item = this._uiRoot.CONFIG.getitem(specification.slice(0, separator));
    const name = specification.slice(separator + 1).toLowerCase();
    // A GUI binding is not an attribute declaration. In particular, it must
    // not persist a made-up zero before a script declares newAttribute(..., 1).
    const attribute = Object.hasOwn(item._attributes, name)
      ? item._attributes[name] : new ConfigAttribute(item, name);
    item._attributes[name] = attribute;
    this._configAttrib = attribute;
    const listener = () => this._cfgAttribChanged(attribute.getdata());
    attribute.on("datachanged", listener);
    bindings.set(this, { attribute, listener });
  };
}
