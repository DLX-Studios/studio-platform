// @bun
// .svelte-kit/output/server/chunks/functions.js
function noop() {}
function once(fn) {
  let done = false;
  let result;
  return () => {
    if (done)
      return result;
    done = true;
    return result = fn();
  };
}
function disallow_on_server(name, parens = "(...)") {
  return () => {
    throw new Error(`Cannot call \`${name}${parens}\` on the server`);
  };
}

// .svelte-kit/output/server/chunks/navigation.js
var afterNavigate = noop;
disallow_on_server("disableScrollHandling", "()");
var goto = disallow_on_server("goto");
var invalidate = disallow_on_server("invalidate");
disallow_on_server("invalidateAll", "()");
disallow_on_server("refreshAll", "()");
disallow_on_server("preloadCode");
disallow_on_server("preloadData");
disallow_on_server("pushState");
disallow_on_server("replaceState");

export { noop, once, afterNavigate, goto, invalidate };

//# debugId=FFB8299D9FDB0F6164756E2164756E21
