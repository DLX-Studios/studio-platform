// @bun
import {
  getContext
} from "./index-e5h3mrsa.js";

// .svelte-kit/output/server/chunks/state.js
function context() {
  return getContext("__request__");
}
var page = {
  get data() {
    return context().page.data;
  },
  get error() {
    return context().page.error;
  },
  get form() {
    return context().page.form;
  },
  get params() {
    return context().page.params;
  },
  get route() {
    return context().page.route;
  },
  get shallow() {
    return context().page.shallow;
  },
  get state() {
    return context().page.state;
  },
  get status() {
    return context().page.status;
  },
  get url() {
    return context().page.url;
  }
};

export { page };

//# debugId=92D9AB38FB1D7F5264756E2164756E21
