// Build-time adaptations to the pinned renderer, shared by build and tests.
// Fail closed if its source changes so these semantic fixes cannot disappear.
function replace(source, before, after) {
  if (!source.includes(before)) throw new Error(`Pinned MAKI source changed: ${before}`);
  return source.replace(before, after);
}

export function adaptMakiResolver(source, servicesModule) {
  return `import { classicProClasses } from ${JSON.stringify(servicesModule)};\n`
    + replace(source, "const klass = GUID_MAP[guid];", "const klass = GUID_MAP[guid] || classicProClasses[guid];");
}

export function adaptMakiSource(kind, source, membersModule) {
  if (kind === "constants") {
    return replace(source, "export const COMMANDS = {", 'export const COMMANDS = {\n  104: { name: "userMember", arg: "type", in: "2", out: "1" },');
  }
  if (kind === "parser") {
    source = replace(source, 'switch (command.arg) {', 'switch (command.arg) {\n    case "type": return "TYPE_CODE";');
    source = replace(source, 'case "VARIABLE_OFFSET":', 'case "TYPE_CODE":\n    case "VARIABLE_OFFSET":');
    source = replace(source, 'makiFile.readUInt8(); // system', 'const isStatic = makiFile.readUInt8() !== 0;');
    source = replace(source, 'variables.push({ type: "OBJECT", value: null, global, guid: klass });', 'variables.push({ type: "OBJECT", value: null, global, guid: klass, isStatic });');
    return source;
  }
  if (kind === "interpreter") {
    source = `import { makiMember, makiInterface } from ${JSON.stringify(membersModule)};\n` + source;
    source = `import { runMakiGenerator, isMakiPromise } from ${JSON.stringify(membersModule.replace(/maki-members\.js$/, "maki-execution.js"))};\n` + source;
    source = replace(source, 'export async function interpret(', 'export function interpret(');
    source = replace(source, 'async interpret(start: number) {', '*interpret(start: number) {');
    source = replace(source, 'return await interpreter.interpret(start);', 'const execution = runMakiGenerator(interpreter.interpret(start));\n    return isMakiPromise(execution) ? execution.catch(error => { console.warn(`Stopped executing ${program.maki_id}.\\n`, error); }) : execution;');
    source = replace(source, 'result = await obj.value[methodName](...methodArgs);', 'result = yield obj.value[methodName](...methodArgs);');
    source = replace(source, '              result = obj.value[methodName](...methodArgs);\n            }', '              result = obj.value[methodName](...methodArgs);\n              if (isMakiPromise(result)) result = yield result;\n            }');
    source = replace(source, 'interpreter.stack = stack;', 'interpreter.stack = stack.map(value => ({ ...value }));');
    source = replace(source, 'current.value = a.value;', 'current.value = current.type === "OBJECT" && current.guid && a.value ? makiInterface(a.value, this.classResolver(current.guid)) : a.value;');
    source = replace(source, 'b.value = a.value;', 'b.value = b.type === "OBJECT" && b.guid && a.value ? makiInterface(a.value, this.classResolver(b.guid)) : a.value;');
    source = replace(source, 'ip = this.callStack.pop();', 'if (this.callStack.length === 0) return this.stack.pop();\n          ip = this.callStack.pop();');
    source = replace(source, 'result = this.variables[1];', 'result = null;');
    source = replace(source, 'if (returnType === "BOOLEAN") {', 'if (returnType === "STRING" && result === null) result = "";\n          if (returnType === "BOOLEAN") {');
    source = replace(source, 'JSON.stringify(methodArgs)', 'JSON.stringify(methodArgs.map(value => value && typeof value === "object" ? `[${value.constructor?.name || "Object"}]` : value))');
    source = replace(source, 'const methodArgs = [];', 'if (methodName === "init") argCount = methodDefinition.parameters.length;\n          const methodArgs = [];');
    source = replace(source, 'if (afunction.constructor.name === "AsyncFunction") {', `if (methodName === "init" && obj.value._div) {
              const parent = methodArgs[0];
              if (!parent || typeof parent.addChild !== "function") throw new Error("MAKI init requires a parent group");
              if (!parent._children.includes(obj.value)) parent.addChild(obj.value);
              obj.value.draw();
              parent.getDiv().appendChild(obj.value.getDiv());
              obj.value.init();
              if (typeof obj.value.afterInited === "function") obj.value.afterInited();
              result = undefined;
            } else if (afunction.constructor.name === "AsyncFunction") {`);
    source = replace(source, 'let result = null;', 'const receiver = makiInterface(obj.value, klass);\n          let result = null;');
    source = replace(source, 'let afunction = obj.value[methodName];', 'let afunction = receiver[methodName];');
    source = replace(source, 'result = yield obj.value[methodName](...methodArgs);', 'result = yield receiver[methodName](...methodArgs);');
    source = replace(source, '              result = obj.value[methodName](...methodArgs);', '              result = receiver[methodName](...methodArgs);');
    return replace(source, "switch (command.opcode) {", `switch (command.opcode) {
        case 104: {
          const name = this.stack.pop();
          const object = this.stack.pop();
          if (!object?.value) throw new Error("MAKI member " + name?.value + " requires an object at instruction " + ip);
          this.push(makiMember(object?.value, this.variables, name?.value, command.arg, this.classes));
          break;
        }`);
  }
  return source;
}
