//#region node_modules/@sveltejs/kit/src/utils/functions.js
function noop() {}
/**
* @template T
* @param {() => T} fn
*/
function once(fn) {
	let done = false;
	/** @type T */
	let result;
	return () => {
		if (done) return result;
		done = true;
		return result = fn();
	};
}
/**
* @param {string} name
* @param {string} [parens]
*/
function disallow_on_server(name, parens = "(...)") {
	return () => {
		throw new Error(`Cannot call \`${name}${parens}\` on the server`);
	};
}
//#endregion
export { noop as n, once as r, disallow_on_server as t };

//# sourceMappingURL=functions.js.map