import { describe, expect, test } from 'bun:test';
import { Window } from 'happy-dom';
import { DomRenderer, admittedAsset, type UiNode } from '../renderer';

const tree = (children: UiNode[]): UiNode => ({ id: 'root', kind: 'column', props: {gap: 8}, children });
const text = (id: string, value: string, props = {}): UiNode => ({id, kind: 'text', props: {text: value, ...props}, children: []});
function setup() {
  const window = new Window();
  const container = window.document.createElement('main');
  window.document.body.append(container);
  const events: string[] = [];
  const renderer = new DomRenderer(container as unknown as HTMLElement, ['assets/sunset.svg'], (id) => events.push(id));
  return {window, container, renderer, events};
}

describe('closed retained Studio DOM adapter', () => {
  test('only admits exact manifest local asset paths', () => {
    const assets = new Set(['assets/sunset.svg', 'assets/a%2fb.svg']);
    expect(admittedAsset('assets/sunset.svg', assets)).toBe('/assets/sunset.svg');
    for (const path of ['https://evil.test/x', '//evil.test/x', 'assets/../secret', 'assets/a%2fb.svg', 'assets/x.svg', 'assets/a\\b.svg']) {
      expect(() => admittedAsset(path, assets)).toThrow();
    }
  });
  test('uses textContent and keeps keyed identity, focus, and fade across commits', () => {
    const {container, renderer, window} = setup();
    const button: UiNode = {id: 'reset', kind: 'button', props: {label: 'Reset', on_pressed: 'pressed'}, children: []};
    renderer.commit(tree([text('clock', '10:00', {opacity: 1, transition: {duration_ms: 700, curve: 'ease_out'}}), button]));
    const clock = container.querySelector('[data-node-id="clock"]')!;
    const reset = container.querySelector('button')!;
    reset.focus();
    renderer.commit(tree([text('clock', '<img onerror=alert(1)>', {opacity: 0, transition: {duration_ms: 700, curve: 'ease_out'}}), button]));
    expect(container.querySelector('[data-node-id="clock"]')).toBe(clock);
    expect(container.querySelector('img')).toBeNull();
    expect(clock.textContent).toBe('<img onerror=alert(1)>');
    expect((clock as unknown as HTMLElement).style.opacity).toBe('0');
    expect((clock as unknown as HTMLElement).style.transition).toContain('700ms');
    expect(window.document.activeElement).toBe(reset);
  });
  test('reorders keyed children and removes stale nodes', () => {
    const {container, renderer} = setup();
    renderer.commit(tree([text('a', 'A'), text('b', 'B')]));
    const b = container.querySelector('[data-node-id="b"]');
    renderer.commit(tree([text('b', 'B2'), text('c', 'C')]));
    expect(container.querySelector('[data-node-id="b"]')).toBe(b);
    expect(container.querySelector('[data-node-id="a"]')).toBeNull();
    expect(container.textContent).toBe('B2C');
  });
  test('rejects unsupported kinds, props, duplicate IDs and unsafe assets atomically', () => {
    const {container, renderer} = setup();
    renderer.commit(tree([text('clock', '10:00')]));
    const before = container.firstElementChild;
    const invalid = [
      tree([{id: 'evil', kind: 'iframe', props: {}, children: []} as unknown as UiNode]),
      tree([text('evil', 'x', {onclick: 'alert(1)'})]),
      tree([text('dup', 'a'), text('dup', 'b')]),
      tree([{id: 'image', kind: 'image', props: {asset: 'assets/missing.svg', alt: 'x'}, children: []}]),
    ];
    for (const next of invalid) {
      expect(() => renderer.commit(next)).toThrow();
      expect(container.firstElementChild).toBe(before);
      expect(container.textContent).toBe('10:00');
    }
  });
  test('hidden subtrees cannot dispatch input, visible buttons dispatch pressed only', () => {
    const {container, renderer, events} = setup();
    const button: UiNode = {id: 'go', kind: 'button', props: {label: 'Go', on_pressed: 'pressed'}, children: []};
    renderer.commit({...tree([button]), props: {visible: false}});
    container.querySelector('button')!.click();
    expect(events).toEqual([]);
    expect(container.querySelector('button')!.disabled).toBe(true);
    renderer.commit(tree([button]));
    container.querySelector('button')!.click();
    expect(events).toEqual(['go']);
  });
  test('safe image sizing and cover artwork stay on native elements', () => {
    const {container, renderer} = setup();
    renderer.commit({id: 'cover', kind: 'box', props: {background_asset: 'assets/sunset.svg'}, children: [
      {id: 'title', kind: 'image', props: {asset: 'assets/sunset.svg', alt: 'Stream starting', width: 900, height: 120}, children: []},
      text('clock', '10:00', {font_size: 168, color: '#FFF5EF'}),
    ]});
    expect((container.firstElementChild as unknown as HTMLElement).style.backgroundSize).toBe('cover');
    expect(container.querySelector('img')!.getAttribute('src')).toBe('/assets/sunset.svg');
    expect(container.querySelector('img')!.alt).toBe('Stream starting');
    expect((container.querySelector('[data-node-id="clock"]') as unknown as HTMLElement).style.fontSize).toBe('168px');
    expect(container.querySelector('[aria-live="assertive"]')).toBeNull();
  });
});

test('compiled Buttons remain interactive when the optional on_pressed property is omitted', () => {
  const {container,renderer,events}=setup();
  renderer.commit(tree([{id:'compiled-button',kind:'button',props:{label:'Go live'},children:[]}]));
  const button=container.querySelector('button')!;
  expect(button.disabled).toBe(false);
  button.click();
  expect(events).toEqual(['compiled-button']);
});
