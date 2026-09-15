export const manifest = (() => {
function __memo(fn) {
	let value;
	return () => value ??= (value = fn());
}

return {
	appDir: "_app",
	appPath: "_app",
	assets: new Set([]),
	mimeTypes: {},
	_: {
		client: {start:"_app/immutable/entry/start.m1pMgU0L.js",app:"_app/immutable/entry/app.B40Rnt5P.js",imports:["_app/immutable/entry/start.m1pMgU0L.js","_app/immutable/entry/payload.DSmR2FwN.js","_app/immutable/chunks/DuqgYnXA.js","_app/immutable/chunks/tW-EaAD7.js","_app/immutable/chunks/DjAkF-xo.js","_app/immutable/chunks/DPBx5xTx.js","_app/immutable/chunks/D4UQy-ZY.js","_app/immutable/chunks/CV8VO5Jt.js","_app/immutable/entry/app.B40Rnt5P.js"],stylesheets:[],fonts:[],uses_env_dynamic_public:false},
		nodes: [
			__memo(() => import('./nodes/0.js')),
			__memo(() => import('./nodes/1.js')),
			__memo(() => import('./nodes/2.js')),
			__memo(() => import('./nodes/3.js')),
			__memo(() => import('./nodes/4.js')),
			__memo(() => import('./nodes/5.js')),
			__memo(() => import('./nodes/6.js')),
			__memo(() => import('./nodes/7.js')),
			__memo(() => import('./nodes/8.js')),
			__memo(() => import('./nodes/9.js')),
			__memo(() => import('./nodes/10.js'))
		],
		remotes: {
			
		},
		routes: [
			{
				id: "/",
				pattern: /^\/$/,
				params: [],
				page: { layouts: [0,], errors: [1,], leaf: 2 },
				endpoint: null
			},
			{
				id: "/agents",
				pattern: /^\/agents\/?$/,
				params: [],
				page: { layouts: [0,], errors: [1,], leaf: 3 },
				endpoint: null
			},
			{
				id: "/analytics",
				pattern: /^\/analytics\/?$/,
				params: [],
				page: { layouts: [0,], errors: [1,], leaf: 4 },
				endpoint: null
			},
			{
				id: "/api/analytics",
				pattern: /^\/api\/analytics\/?$/,
				params: [],
				page: null,
				endpoint: __memo(() => import('./entries/endpoints/api/analytics/_server.ts.js'))
			},
			{
				id: "/api/auth/login",
				pattern: /^\/api\/auth\/login\/?$/,
				params: [],
				page: null,
				endpoint: __memo(() => import('./entries/endpoints/api/auth/login/_server.ts.js'))
			},
			{
				id: "/api/auth/logout",
				pattern: /^\/api\/auth\/logout\/?$/,
				params: [],
				page: null,
				endpoint: __memo(() => import('./entries/endpoints/api/auth/logout/_server.ts.js'))
			},
			{
				id: "/api/daemon/hello",
				pattern: /^\/api\/daemon\/hello\/?$/,
				params: [],
				page: null,
				endpoint: __memo(() => import('./entries/endpoints/api/daemon/hello/_server.ts.js'))
			},
			{
				id: "/api/do/options",
				pattern: /^\/api\/do\/options\/?$/,
				params: [],
				page: null,
				endpoint: __memo(() => import('./entries/endpoints/api/do/options/_server.ts.js'))
			},
			{
				id: "/api/do/sshkeys",
				pattern: /^\/api\/do\/sshkeys\/?$/,
				params: [],
				page: null,
				endpoint: __memo(() => import('./entries/endpoints/api/do/sshkeys/_server.ts.js'))
			},
			{
				id: "/api/do/sshkeys/generate",
				pattern: /^\/api\/do\/sshkeys\/generate\/?$/,
				params: [],
				page: null,
				endpoint: __memo(() => import('./entries/endpoints/api/do/sshkeys/generate/_server.ts.js'))
			},
			{
				id: "/api/machines",
				pattern: /^\/api\/machines\/?$/,
				params: [],
				page: null,
				endpoint: __memo(() => import('./entries/endpoints/api/machines/_server.ts.js'))
			},
			{
				id: "/api/machines/[id]",
				pattern: /^\/api\/machines\/([^/]+?)\/?$/,
				params: [{"name":"id","optional":false,"rest":false,"chained":false}],
				page: null,
				endpoint: __memo(() => import('./entries/endpoints/api/machines/_id_/_server.ts.js'))
			},
			{
				id: "/api/machines/[id]/agent",
				pattern: /^\/api\/machines\/([^/]+?)\/agent\/?$/,
				params: [{"name":"id","optional":false,"rest":false,"chained":false}],
				page: null,
				endpoint: __memo(() => import('./entries/endpoints/api/machines/_id_/agent/_server.ts.js'))
			},
			{
				id: "/api/setup",
				pattern: /^\/api\/setup\/?$/,
				params: [],
				page: null,
				endpoint: __memo(() => import('./entries/endpoints/api/setup/_server.ts.js'))
			},
			{
				id: "/api/setup/probe",
				pattern: /^\/api\/setup\/probe\/?$/,
				params: [],
				page: null,
				endpoint: __memo(() => import('./entries/endpoints/api/setup/probe/_server.ts.js'))
			},
			{
				id: "/api/setup/spaces/probe",
				pattern: /^\/api\/setup\/spaces\/probe\/?$/,
				params: [],
				page: null,
				endpoint: __memo(() => import('./entries/endpoints/api/setup/spaces/probe/_server.ts.js'))
			},
			{
				id: "/login",
				pattern: /^\/login\/?$/,
				params: [],
				page: { layouts: [0,], errors: [1,], leaf: 5 },
				endpoint: null
			},
			{
				id: "/machines",
				pattern: /^\/machines\/?$/,
				params: [],
				page: { layouts: [0,], errors: [1,], leaf: 7 },
				endpoint: null
			},
			{
				id: "/m/[id]",
				pattern: /^\/m\/([^/]+?)\/?$/,
				params: [{"name":"id","optional":false,"rest":false,"chained":false}],
				page: { layouts: [0,], errors: [1,], leaf: 6 },
				endpoint: null
			},
			{
				id: "/providers",
				pattern: /^\/providers\/?$/,
				params: [],
				page: { layouts: [0,], errors: [1,], leaf: 8 },
				endpoint: null
			},
			{
				id: "/setup",
				pattern: /^\/setup\/?$/,
				params: [],
				page: { layouts: [0,], errors: [1,], leaf: 9 },
				endpoint: null
			},
			{
				id: "/ssh-keys",
				pattern: /^\/ssh-keys\/?$/,
				params: [],
				page: { layouts: [0,], errors: [1,], leaf: 10 },
				endpoint: null
			}
		],
		prerendered_routes: new Set([]),
		matchers: async () => {
			return {};
		},
		server_assets: {}
	}
}
})();
