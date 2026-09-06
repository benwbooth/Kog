export function isMakiPromise(value) {
  return value != null && typeof value.then === "function";
}

// Wasabi callbacks are synchronous. Only suspend execution for a genuinely
// asynchronous host operation, not after every ordinary script callback.
export function runMakiGenerator(iterator) {
  function advance(method, value) {
    let step = iterator[method](value);
    while (!step.done) {
      if (isMakiPromise(step.value)) {
        return step.value.then(
          result => advance("next", result),
          error => advance("throw", error),
        );
      }
      step = iterator.next(step.value);
    }
    return step.value;
  }
  return advance("next");
}
