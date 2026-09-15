import { redirect } from "@sveltejs/kit";
//#region src/routes/+page.ts
var load = () => {
	redirect(302, "/machines");
};
//#endregion
export { load };

//# sourceMappingURL=_page.ts.js.map