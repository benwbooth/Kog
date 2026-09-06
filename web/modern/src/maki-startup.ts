import Vm from "../../../native/webamp/packages/webamp-modern/src/skin/VM";
import Timer from "../../../native/webamp/packages/webamp-modern/src/skin/makiClasses/Timer";
import Container from "../../../native/webamp/packages/webamp-modern/src/skin/makiClasses/Container";

type Deferred = { vm: any; object: any; event: string; args: any[] };
type Startup = {
  pending: Set<Promise<unknown>>;
  timers: Set<Timer>;
  deferred: Deferred[];
  initialVisibility: Set<object>;
  notifyingVisibility: boolean;
};
const startups = new WeakMap<object, Startup>();
const startTimer = Timer.prototype.start;
let originalDispatch: typeof Vm.prototype.dispatch;
let installed = false;
const deferredEvents = new Set(["onresize", "onstartup"]);
const eventBudgets = new WeakMap<object, { since: number; count: number; counts: Map<string, number>; blocked: string | null }>();

async function flushDeferred(startup: Startup) {
  let count = 0;
  const counts = new Map<string, number>();
  while (startup.deferred.length) {
    const next = startup.deferred.shift()!;
    const key = `${next.event}:${next.object.getId?.()}`;
    counts.set(key, (counts.get(key) ?? 0) + 1);
    if (++count > 4096) throw new Error("MAKI startup event limit exceeded: " + [...counts.entries()].sort((a, b) => b[1] - a[1]).slice(0, 6).map(entry => entry.join("=")).join(", "));
    await originalDispatch.call(next.vm, next.object, next.event, next.args);
  }
}

async function notifyLayout(root: any, container: any, event: string, layout: unknown) {
  if (!layout) return;
  const args: any[] = [{ type: "OBJECT", value: layout }];
  await root.vm.dispatch(container, event, args);
  const systems = new Set(root.vm._scripts.map((script: any) => script.variables[0].value));
  for (const system of systems) await root.vm.dispatch(system, event, args);
}

export function beginMakiStartup(root: object) {
  startups.set(root, { pending: new Set(), timers: new Set(), deferred: [], initialVisibility: new Set(), notifyingVisibility: false });
}

async function notifyInitialVisibility(root: any, startup: Startup) {
  // Wasabi BaseWnd::onPostOnInit notifies initially visible windows, even
  // without a visibility transition. ClassicPro uses this to start its tab
  // selection timer. Wait until onScriptLoaded bindings are initialized.
  const visited = new Set<object>();
  const visit = async (object: any) => {
    if (!object || visited.has(object)) return;
    if (visited.size >= 50000) throw new Error("MAKI initial visibility tree limit exceeded");
    visited.add(object);
    if (!object.isvisible()) return;
    if (!startup.initialVisibility.has(object)) {
      startup.initialVisibility.add(object);
      await root.vm.dispatch(object, "onsetvisible", [{ type: "BOOLEAN", value: 1 }]);
    }
    for (const child of [...(object._children ?? [])]) await visit(child);
  };
  startup.notifyingVisibility = true;
  try {
    for (const container of [...root.getContainers()]) {
      if (container.getVisible()) await visit(container.getcurlayout());
    }
  } finally {
    startup.notifyingVisibility = false;
  }
}

export async function finishMakiStartup(root: any) {
  const startup = startups.get(root);
  if (!startup) return;
  for (let round = 0; startup.pending.size; round++) {
    if (round >= 128) throw new Error("MAKI startup did not settle");
    await Promise.all([...startup.pending]);
  }
  await flushDeferred(startup);
  for (const container of [...root.getContainers()]) {
    if (container.getVisible()) await notifyLayout(root, container, "onshowlayout", container.getcurlayout());
  }
  await notifyInitialVisibility(root, startup);
}

export async function resumeMakiTimers(root: object) {
  const startup = startups.get(root);
  if (!startup) return;
  for (let round = 0; startup.pending.size; round++) {
    if (round >= 128) throw new Error("MAKI late startup did not settle");
    await Promise.all([...startup.pending]);
  }
  await flushDeferred(startup);
  startups.delete(root);
  for (const timer of startup.timers) startTimer.call(timer);
}

export function installMakiStartup() {
  if (installed) return;
  installed = true;
  originalDispatch = Vm.prototype.dispatch;
  const switchLayout = Container.prototype.switchtolayout;
  Container.prototype.switchtolayout = async function (id: string) {
    const previous = this.getcurlayout();
    switchLayout.call(this, id);
    if (!startups.has(this._uiRoot) && previous !== this.getcurlayout() && this.getVisible()) {
      await notifyLayout(this._uiRoot, this, "onhidelayout", previous);
      await notifyLayout(this._uiRoot, this, "onshowlayout", this.getcurlayout());
    }
  };
  (Vm.prototype as any).dispatch = function (object: unknown, event: string, args: any[] = []) {
    const startup = startups.get(this._uiRoot);
    // An ancestor's initialization handler can show a child. That transition
    // already delivers its visibility; do not deliver it again while walking.
    if (startup?.notifyingVisibility && event === "onsetvisible" && object && args[0]?.value) {
      startup.initialVisibility.add(object as object);
    }
    if (!startup) {
      let budget = eventBudgets.get(this);
      const now = performance.now();
      if (!budget || (!budget.blocked && now - budget.since > 1000)) {
        budget = { since: now, count: 0, counts: new Map(), blocked: null };
        eventBudgets.set(this, budget);
      }
      if (budget.blocked) throw new Error(budget.blocked);
      const key = `${event}:${(object as any)?.getId?.()}`;
      budget.counts.set(key, (budget.counts.get(key) ?? 0) + 1);
      if (++budget.count > 10000) {
        budget.blocked = "MAKI event storm: " + [...budget.counts.entries()].sort((a, b) => b[1] - a[1]).slice(0, 4).map(entry => entry.join("=")).join(", ");
        throw new Error(budget.blocked);
      }
    }
    if (startup && deferredEvents.has(event)) {
      if (startup.deferred.length >= 4096) throw new Error("MAKI deferred event limit exceeded");
      const copy = args.map(value => ({ ...value }));
      // Resize events are state notifications; only the latest geometry is
      // relevant before script startup has completed.
      const prior = event === "onresize" ? startup.deferred.find(item => item.object === object && item.event === event) : null;
      if (prior) prior.args = copy;
      else startup.deferred.push({ vm: this, object, event, args: copy });
      return Promise.resolve(0);
    }
    const result = originalDispatch.call(this, object, event, args);
    if (startup && event === "onscriptloaded") {
      const pending = Promise.resolve(result);
      startup.pending.add(pending);
      void pending.then(() => startup.pending.delete(pending), () => startup.pending.delete(pending));
    }
    return result;
  };
  Timer.prototype.start = function () {
    const startup = startups.get(this._uiRoot);
    if (!startup) return startTimer.call(this);
    if (!this._delay) return false;
    startup.timers.add(this);
    return true;
  };
  const stop = Timer.prototype.stop;
  Timer.prototype.stop = function () {
    startups.get(this._uiRoot)?.timers.delete(this);
    return stop.call(this);
  };
  const running = Timer.prototype.isrunning;
  Timer.prototype.isrunning = function () {
    return startups.get(this._uiRoot)?.timers.has(this) || running.call(this);
  };
}
