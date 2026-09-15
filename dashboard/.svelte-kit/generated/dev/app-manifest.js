// empty during dev
export const immutable = [];
export const prerendered = [];

export const assets = [
	
];

export const routes = [
	{"id":"/","page":true,"endpoint":false},
	{"id":"/agents","page":true,"endpoint":false},
	{"id":"/analytics","page":true,"endpoint":false},
	{"id":"/api/analytics","page":false,"endpoint":true},
	{"id":"/api/auth/login","page":false,"endpoint":true},
	{"id":"/api/auth/logout","page":false,"endpoint":true},
	{"id":"/api/daemon/hello","page":false,"endpoint":true},
	{"id":"/api/do/options","page":false,"endpoint":true},
	{"id":"/api/do/sshkeys","page":false,"endpoint":true},
	{"id":"/api/do/sshkeys/generate","page":false,"endpoint":true},
	{"id":"/api/machines","page":false,"endpoint":true},
	{"id":"/api/machines/[id]","page":false,"endpoint":true},
	{"id":"/api/machines/[id]/agent","page":false,"endpoint":true},
	{"id":"/api/setup","page":false,"endpoint":true},
	{"id":"/api/setup/probe","page":false,"endpoint":true},
	{"id":"/api/setup/spaces/probe","page":false,"endpoint":true},
	{"id":"/login","page":true,"endpoint":false},
	{"id":"/machines","page":true,"endpoint":false},
	{"id":"/m/[id]","page":true,"endpoint":false},
	{"id":"/providers","page":true,"endpoint":false},
	{"id":"/setup","page":true,"endpoint":false},
	{"id":"/ssh-keys","page":true,"endpoint":false}
];