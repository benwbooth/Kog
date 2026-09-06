import Vm from "../../../native/webamp/packages/webamp-modern/src/skin/VM";
import { interpret } from "../../../native/webamp/packages/webamp-modern/src/maki/interpreter";
import { classResolver } from "../../../native/webamp/packages/webamp-modern/src/skin/resolver";
import { runMakiGenerator, isMakiPromise } from "./maki-execution.js";

export function installMakiDispatch() {
  (Vm.prototype as any).interpret = function (script, offset, event, args) {
    return interpret(offset, script, args, classResolver, event, this._uiRoot);
  };
  (Vm.prototype as any).dispatch = function (object, event, args = []) {
    const vm = this;
    return runMakiGenerator((function* () {
      const reversed = [...args].reverse();
      let executed = 0;
      for (const script of vm._scripts) {
        for (const binding of script.bindings) {
          if (script.methods[binding.methodOffset].name !== event) continue;
          const variable = script.variables[binding.variableOffset];
          const matches = variable.isClass
            ? variable.members.some(index => script.variables[index].value === object)
            : variable.type === "OBJECT" && variable.value === object;
          if (!matches) continue;
          if (variable.isClass) variable.value = object;
          const result = vm.interpret(script, binding.commandOffset, event, reversed);
          if (isMakiPromise(result)) yield result;
          executed += 1;
        }
      }
      return executed;
    })());
  };
}
