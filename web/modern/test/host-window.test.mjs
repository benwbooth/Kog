import assert from "node:assert/strict";
import { test } from "node:test";
import esbuild from "esbuild";

test("main host window bridges geometry and native gestures without moving auxiliary containers", async () => {
  const result = await esbuild.build({ entryPoints: [new URL("../src/host-window.ts", import.meta.url).pathname],
    bundle: true, format: "esm", write: false });
  const { installHostWindow } = await import(`data:text/javascript;base64,${Buffer.from(result.outputFiles[0].contents).toString("base64")}`);
  const old = { window: globalThis.window, document: globalThis.document, ResizeObserver: globalThis.ResizeObserver };
  const events = {}, calls = [];
  let observe;
  globalThis.window = { addEventListener(name, handler) { events[name] = handler; },
    setInterval(handler) { events.refresh = handler; return 1; }, clearInterval() {}, removeEventListener() {} };
  globalThis.document = { documentElement: { clientWidth: 800, clientHeight: 600 } };
  globalThis.ResizeObserver = class { constructor(callback) { observe = callback; } observe() {} disconnect() {} };
  try {
    const layout = { w: 500, h: 400, _minimumWidth: 317, _minimumHeight: 168,
      getwidth() { return this.w; }, getheight() { return this.h; }, getDiv() { return {}; },
      resize(x,y,w,h) { this.w=w; this.h=h; }, _invalidateSize() { this.invalidated=true; },
      setResizing(command, mask) { this._canResize=mask; }, setMoving() { throw Error("must not translate DOM"); } };
    const main = { _layouts: [layout], getcurlayout() { return layout; }, setLocation(x,y) { assert.deepEqual([x,y],[0,0]); } };
    const root = { getContainers() { return [main]; }, findContainer(id) { assert.equal(id,"main"); return main; }, dispatch() {} };
    installHostWindow(root, (name,data) => calls.push([name,data]));
    assert.deepEqual(calls[0], ["windowGeometry", { width:500,height:400,minimumWidth:317,minimumHeight:168,maximumWidth:16384,maximumHeight:16384 }]);
    events.resize();
    assert.equal(layout.w,800); assert.equal(layout.h,600); assert.ok(layout.invalidated);
    const count=calls.length; observe(); assert.equal(calls.length,count, "unchanged geometry cannot loop");
    layout.setMoving("start",0,0); layout.setMoving("move",200,100);
    assert.equal(calls.at(-1)[0],"windowMove");
    layout.setResizing("constraint",20,0); layout.setResizing("start",0,0);
    assert.deepEqual(calls.at(-1),["windowResize",{left:false,right:true,top:false,bottom:true}]);
    root.dispatch("MINIMIZE"); assert.equal(calls.at(-1)[0],"windowMinimize");
    events.refresh(); assert.equal(calls.at(-1)[0], "windowGeometry", "retry dropped startup geometry");
    events.pagehide();
    root.getContainers = () => [main, { getVisible() { return true; } }];
    const beforeDesktop = calls.length;
    installHostWindow(root, (name,data) => calls.push([name,data]));
    assert.equal(calls.length, beforeDesktop, "multi-container canvas is not shrunk");
    layout.setMoving("start",0,0);
    assert.equal(calls.at(-1)[0], "windowMove", "auxiliary windows do not disable title-bar dragging");
  } finally { Object.assign(globalThis,old); }
});
