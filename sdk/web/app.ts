import {DomRenderer} from './renderer';
import type {NavigationCommand, WebConfig, WorkerReply, WorkerRequest} from './worker-protocol';
type RequestBody = WorkerRequest extends infer R ? R extends {id: number} ? Omit<R, 'id'> : never : never;
interface WorkerPort {postMessage(message: WorkerRequest): void; terminate(): void}

/** One active guest request; unrelated messages never renew its watchdog. */
export class RequestQueue {
  private pending: RequestBody[] = [];
  private active: WorkerRequest | null = null;
  private deadline: ReturnType<typeof setTimeout> | undefined;
  private nextId = 1;
  private stopped = false;
  constructor(private readonly worker: WorkerPort, private readonly commit: (reply: Extract<WorkerReply, {type: 'commit'}>) => void,
    private readonly failure: (message: string) => void, private readonly timeoutMs = 2000, private readonly startupMs = 10000) {}
  send(request: RequestBody): void {
    if (this.stopped) return;
    if (request.type === 'poll' && (this.active?.type === 'poll' || this.pending.some(item => item.type === 'poll'))) return;
    if (this.pending.length >= 64) {this.fail('Too many pending Studio requests. Reload to restart.'); return;}
    this.pending.push(request);
    this.advance();
  }
  receive(value: unknown): void {
    if (this.stopped || !this.active || typeof value !== 'object' || value === null || !('id' in value) || value.id !== this.active.id) return;
    const reply = value as WorkerReply;
    if (!['commit', 'idle', 'error'].includes(reply.type)) {this.fail('Invalid Studio worker response.'); return;}
    if (reply.type === 'error') {this.fail(typeof reply.message === 'string' ? reply.message : 'Studio worker failed.'); return;}
    try {
      if (reply.type === 'commit') {
        if (typeof reply.route !== 'string' || !reply.route.startsWith('/') || reply.route.startsWith('//')
          || /[?#\\\u0000-\u0020]/.test(reply.route) || ![null, 'push', 'replace', 'pop', 'pop_to', 'reset'].includes(reply.history_operation)) throw new Error('Invalid committed route');
        this.commit(reply);
      }
    } catch (error) {this.fail(error instanceof Error ? error.message : 'Invalid Studio commit.'); return;}
    clearTimeout(this.deadline);
    this.active = null;
    this.advance();
  }
  stop(): void {
    if (this.stopped) return;
    this.stopped = true;
    clearTimeout(this.deadline);
    this.pending = [];
    this.active = null;
    this.worker.terminate();
  }
  private advance(): void {
    if (this.active || this.stopped) return;
    const request = this.pending.shift();
    if (!request) return;
    this.active = {...request, id: this.nextId++} as WorkerRequest;
    this.deadline = setTimeout(() => this.fail('The Studio worker stopped responding. Reload to restart.'), request.type === 'init' ? this.startupMs : this.timeoutMs);
    try {this.worker.postMessage(this.active);} catch {this.fail('Could not contact the Studio worker.');}
  }
  private fail(message: string): void {this.stop(); this.failure(message);}
}

interface HistoryMark {studioSession: string; index: number; route: string}
/** Reconciles browser history only after a host-admitted, rendered commit. */
class BrowserHistory {
  private session = `${Date.now()}-${Math.random()}`;
  private index = 0;
  private route = location.pathname;
  private expectedPop: string | null = null;
  private browserMove = false;
  constructor(private readonly navigate: (command: NavigationCommand) => void) {
    history.replaceState(this.mark(), '', this.route);
    addEventListener('popstate', (event: PopStateEvent) => {
      const target = location.pathname;
      const mark = event.state as HistoryMark | null;
      if (this.expectedPop === target) {
        this.expectedPop = null;
        this.index = mark?.studioSession === this.session ? mark.index : Math.max(0, this.index - 1);
        this.route = target;
        return;
      }
      this.expectedPop = null;
      this.browserMove = true;
      const sameSession = mark?.studioSession === this.session;
      const operation = sameSession && mark.index < this.index ? 'pop_to' : sameSession && mark.index > this.index ? 'push' : 'reset';
      this.index = sameSession ? mark.index : 0;
      if (!sameSession) this.session = `${Date.now()}-${Math.random()}`;
      this.navigate({operation, route: target});
    });
  }
  commit(route: string, operation: NavigationCommand['operation'] | null): void {
    this.route = route;
    if (this.browserMove) {this.browserMove = false; history.replaceState(this.mark(), '', route); return;}
    if (operation === 'push') {this.index++; history.pushState(this.mark(), '', route);}
    else if (operation === 'pop' && this.index > 0) {this.expectedPop = route; history.back();}
    else {
      if (operation === 'reset') {this.session = `${Date.now()}-${Math.random()}`; this.index = 0;}
      history.replaceState(this.mark(), '', route);
    }
  }
  private mark(): HistoryMark {return {studioSession: this.session, index: this.index, route: this.route};}
}

export async function startApp(container: HTMLElement, status: HTMLElement): Promise<void> {
  let queue: RequestQueue | undefined;
  let poll: ReturnType<typeof setInterval> | undefined;
  const failure = (message: string) => {
    if (poll !== undefined) clearInterval(poll);
    container.removeAttribute('aria-busy');
    status.textContent = `${message} The last screen has been kept.`;
    status.hidden = false;
  };
  try {
    const response = await fetch('/web-config.json', {credentials: 'same-origin'});
    if (!response.ok) throw new Error('Could not load the Studio application.');
    const source = await response.text();
    if (source.length > 2 * 1024 * 1024) throw new Error('Studio configuration is too large.');
    const config = JSON.parse(source) as WebConfig;
    if (config.schemaVersion !== 1 || !Array.isArray(config.assets) || config.assets.length > 5000 || typeof config.title !== 'string') throw new Error('Invalid Studio application configuration.');
    document.title = config.title;
    const renderer = new DomRenderer(container, config.assets, node_id => queue?.send({type: 'input', node_id, event: 'pressed'}));
    const worker = new Worker('/worker.js', {type: 'module', name: 'studio-guest'});
    const navigation = new BrowserHistory(command => queue?.send({type: 'navigate', command}));
    queue = new RequestQueue(worker, reply => {
      renderer.commit(reply.root);
      navigation.commit(reply.route, reply.history_operation);
      container.removeAttribute('aria-busy');
      status.textContent = '';
      status.hidden = true;
    }, failure);
    worker.addEventListener('message', event => queue?.receive(event.data));
    worker.addEventListener('error', () => {queue?.stop(); failure('The Studio worker failed. Reload to restart.');});
    worker.addEventListener('messageerror', () => {queue?.stop(); failure('Invalid Studio worker message.');});
    queue.send({type: 'init', route: location.pathname});
    poll = setInterval(() => {if (!document.hidden) queue?.send({type: 'poll'});}, 100);
    addEventListener('pagehide', () => {if (poll !== undefined) clearInterval(poll); queue?.stop();}, {once: true});
  } catch (error) {queue?.stop(); failure(error instanceof Error ? error.message : 'Could not start the Studio application.');}
}

if (typeof document !== 'undefined') {
  const container = document.querySelector<HTMLElement>('[data-studio-root]');
  const status = document.querySelector<HTMLElement>('[data-studio-status]');
  if (container && status) void startApp(container, status);
}
