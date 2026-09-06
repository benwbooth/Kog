// Public MAKI settings are shared by scripts, but never expose host settings.
export class MakiPreferences {
  constructor(storage) { this.storage = storage; }
  key(item) { return `kog.maki.public:${String(item).toLowerCase()}`; }
  getString(item, fallback) {
    return this.storage.getItem(this.key(item)) ?? fallback;
  }
  setString(item, value) { this.storage.setItem(this.key(item), String(value)); }
  getInt(item, fallback) {
    const value = this.storage.getItem(this.key(item));
    if (value === null) return fallback;
    const number = Number.parseInt(value, 10);
    return Number.isFinite(number) ? number | 0 : 0;
  }
  setInt(item, value) { this.setString(item, Number(value) | 0); }
}

// Wasabi bfc/bitlist.h: out-of-range reads are false and writes do not grow.
export function installBitListApi(BitList) {
  BitList.prototype.getitem = function (index) {
    return Number.isInteger(index) && index >= 0 && index < this._items.length ? Boolean(this._items[index]) : false;
  };
  BitList.prototype.setitem = function (index, value) {
    if (Number.isInteger(index) && index >= 0 && index < this._items.length) this._items[index] = Boolean(value);
  };
  BitList.prototype.setsize = function (size) {
    if (!Number.isInteger(size) || size < 0 || size > 1048576) throw new Error("Invalid MAKI BitList size");
    const previous = this._items.length;
    this._items.length = size;
    if (size > previous) this._items.fill(false, previous);
  };
}
