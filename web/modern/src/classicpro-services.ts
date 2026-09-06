import BaseObject from "../../../native/webamp/packages/webamp-modern/src/skin/makiClasses/BaseObject";
import SystemObject from "../../../native/webamp/packages/webamp-modern/src/skin/makiClasses/SystemObject";
import { getClass, getFormattedId, normalizedObjects } from "../../../native/webamp/packages/webamp-modern/src/maki/objects";
import { classicProColorClasses, bindClassicProColorGlobals } from "./classicpro-colors";
import BitList from "../../../native/webamp/packages/webamp-modern/src/skin/makiClasses/BitList";
import PRIVATE_CONFIG from "../../../native/webamp/packages/webamp-modern/src/skin/PrivateConfig";
import { MakiPreferences, installBitListApi } from "./maki-preferences.js";

// The native service's ABI is referenced by the original ClassicPro scripts.
// Kog deliberately denies OS execution and unrestricted directory access. These
// methods fail explicitly instead of treating a blocked operation as success.
class ClassicProFile extends BaseObject {
  static GUID = "9d822ff44c7a9f86023ceb8ca5d520fe";

  explorefile(_path: string) {
    throw new Error("ClassicPro Explorer actions are unavailable in Kog's skin sandbox.");
  }
  openfile(_path: string) {
    throw new Error("ClassicPro cannot launch programs or widget uninstallers from Kog's skin sandbox.");
  }
  findfiles(_path: string, _pattern: string, _results: unknown) {
    throw new Error("ClassicPro folder scanning is unavailable in Kog's skin sandbox. Open files using Kog's file picker.");
  }
}

class WinampPrivate extends BaseObject {
  static GUID = "78bd6ed94fa50dbc7759cdb5f812a9e3";
  updatelinks(_version: string, _browserVersion: string) {
    throw new Error("Winamp browser link-list downloads are unavailable in Kog's skin sandbox.");
  }
}

export const classicProClasses = { [ClassicProFile.GUID]: ClassicProFile, [WinampPrivate.GUID]: WinampPrivate, ...classicProColorClasses };

export function installClassicProServices() {
  normalizedObjects[getFormattedId(WinampPrivate.GUID)] = {
    name: "Private", parent: "Object", parentClass: getClass(BaseObject.GUID),
    functions: [
      { name: "updateLinks", result: "", parameters: [["String", "version"], ["String", "browserVersion"]] },
      { name: "onLinksUpdated", result: "", parameters: [] },
    ],
  };
  SystemObject.prototype.getdate = () => Math.floor(Date.now() / 1000);
  SystemObject.prototype.getdateyear = (value) => new Date(value * 1000).getFullYear() - 1900;
  SystemObject.prototype.getdatemonth = (value) => new Date(value * 1000).getMonth();
  SystemObject.prototype.getdateday = (value) => new Date(value * 1000).getDate();
  SystemObject.prototype.getdatedow = (value) => new Date(value * 1000).getDay();
  SystemObject.prototype.getdatedoy = (value) => {
    const date = new Date(value * 1000);
    return Math.floor((Date.UTC(date.getFullYear(), date.getMonth(), date.getDate()) - Date.UTC(date.getFullYear(), 0, 1)) / 86400000);
  };
  SystemObject.prototype.getdatehour = (value) => new Date(value * 1000).getHours();
  SystemObject.prototype.getdatemin = (value) => new Date(value * 1000).getMinutes();
  SystemObject.prototype.getdatesec = (value) => new Date(value * 1000).getSeconds();
  const preferences = new MakiPreferences(window.localStorage);
  SystemObject.prototype.getpublicint = (item, fallback) => preferences.getInt(item, fallback);
  SystemObject.prototype.getpublicstring = (item, fallback) => preferences.getString(item, fallback);
  SystemObject.prototype.setpublicint = (item, value) => preferences.setInt(item, value);
  SystemObject.prototype.setpublicstring = (item, value) => preferences.setString(item, value);
  SystemObject.prototype.setprivatestring = (section, item, value) => { PRIVATE_CONFIG.setPrivateString(section, item, value); };
  installBitListApi(BitList);
  normalizedObjects[getFormattedId(ClassicProFile.GUID)] = {
    name: "ClassicProFile", parent: "Object", parentClass: getClass(BaseObject.GUID),
    functions: [
      { name: "exploreFile", result: "", parameters: [["String", "path"]] },
      { name: "openFile", result: "", parameters: [["String", "path"]] },
      { name: "findFiles", result: "int", parameters: [["String", "path"], ["String", "pattern"], ["List", "results"]] },
    ],
  };
  const originalInit = SystemObject.prototype.init;
  SystemObject.prototype.init = function () {
    bindClassicProColorGlobals(this);
    for (const variable of this._parsedScript.variables) {
      if (variable.type === "OBJECT" && variable.guid === ClassicProFile.GUID && (variable as any).isStatic === true) {
        variable.value = new ClassicProFile();
      }
    }
    return originalInit.call(this);
  };
}
