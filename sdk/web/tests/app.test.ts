import {expect, test} from 'bun:test';
import {RequestQueue} from '../app';

test('serializes Worker requests and ignores unsolicited replies without renewing deadline', async () => {
  const sent: {id: number; type: string}[] = [];
  let terminated = 0;
  const commits: unknown[] = [];
  const errors: string[] = [];
  const queue = new RequestQueue({postMessage: (message: unknown) => sent.push(message as typeof sent[number]), terminate: () => terminated++}, reply => commits.push(reply), error => errors.push(error), 20, 20);
  queue.send({type: 'init', route: '/'});
  queue.send({type: 'poll'});
  expect(sent.length).toBe(1);
  queue.receive({id: 999, type: 'idle'});
  await new Promise(resolve => setTimeout(resolve, 35));
  expect(terminated).toBe(1);
  expect(sent.length).toBe(1);
  expect(commits).toEqual([]);
  expect(errors.length).toBe(1);
});

test('admits only matching replies and advances FIFO once per successful request', () => {
  const sent: {id: number; type: string}[] = [];
  const queue = new RequestQueue({postMessage: (message: unknown) => sent.push(message as typeof sent[number]), terminate: () => {}}, () => {}, () => {}, 1000, 1000);
  queue.send({type: 'init', route: '/'});
  queue.send({type: 'input', node_id: 'reset', event: 'pressed'});
  queue.receive({id: sent[0]!.id, type: 'idle'});
  expect(sent.map(message => message.type)).toEqual(['init', 'input']);
  queue.receive({id: sent[0]!.id, type: 'idle'});
  expect(sent.length).toBe(2);
  queue.receive({id: sent[1]!.id, type: 'idle'});
  queue.stop();
});
