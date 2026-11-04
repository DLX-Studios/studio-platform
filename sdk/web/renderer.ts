/** Closed, retained DOM adapter for the static Studio web target. */
export interface UiNode {
  id: string;
  kind: string;
  props: Record<string, unknown>;
  children: UiNode[];
}
import contract from './contract.json';
const kinds = new Set(Object.keys(contract.kinds));
const encoder = new TextEncoder();
const common: readonly string[] = contract.common;
const specific: Record<string, readonly string[]> = contract.kinds;
const colors: Record<string, string> = {surface: '#251c34', surface_variant: '#39233f', primary: '#ff72b0', secondary: '#d8b5ee', error: '#ff7d8f', success: '#99ebc2', warning: '#ffe09b', transparent: 'transparent'};
const curves: Record<string, string> = {linear: 'linear', ease_in: 'ease-in', ease_out: 'ease-out', ease_in_out: 'ease-in-out'};
const record = (value: unknown): value is Record<string, unknown> => typeof value === 'object' && value !== null && !Array.isArray(value);
const finite = (value: unknown, min = 0, max = Number.MAX_SAFE_INTEGER): value is number => typeof value === 'number' && Number.isFinite(value) && value >= min && value <= max;
const boundedString = (value: unknown, nonempty = false): value is string => typeof value === 'string' && (!nonempty || value.length > 0) && encoder.encode(value).length <= 65536;

/** Asset URLs are rooted at the static export, including on deep-linked routes. */
export function admittedAsset(value: unknown, assets: ReadonlySet<string>): string {
  if (typeof value !== 'string' || !assets.has(value) || !/^assets\/[A-Za-z0-9_./ -]+$/.test(value)
    || value.split('/').some(segment => !segment || segment === '.' || segment === '..')) {
    throw new Error('Unadmitted local asset');
  }
  return `/${value.split('/').map(segment => encodeURIComponent(segment)).join('/')}`;
}

function validProperty(kind: string, key: string, value: unknown, assets: ReadonlySet<string>): boolean {
  if (!common.includes(key) && !specific[kind]?.includes(key)) return false;
  switch (key) {
    case 'visible': case 'enabled': case 'shrink': return typeof value === 'boolean';
    case 'opacity': return finite(value, 0, 1);
    case 'flex': return finite(value, 0, 10);
    case 'font_size': return finite(value, 8, 256);
    case 'padding': case 'gap': case 'height': return finite(value);
    case 'width': return kind === 'button' ? value === 'intrinsic' || value === 'full' : finite(value);
    case 'color': return typeof value === 'string' && /^#[a-fA-F0-9]{6}$/.test(value);
    case 'background': return typeof value === 'string' && Object.hasOwn(colors, value);
    case 'asset': case 'background_asset': admittedAsset(value, assets); return true;
    case 'alignment': return ['start', 'center', 'end', 'stretch', 'space_between'].includes(value as string);
    case 'typography_role': return ['body', 'label', 'caption', 'title', 'headline', 'display'].includes(value as string);
    case 'variant': return (kind === 'button' ? ['primary', 'secondary', 'selected'] : ['default', 'success', 'warning', 'destructive']).includes(value as string);
    case 'text': return boundedString(value);
    case 'label': case 'alt': case 'accessibility_label': return boundedString(value, true);
    case 'on_pressed': return typeof value === 'string' && /^[A-Za-z0-9_.:-]{1,128}$/.test(value);
    case 'transition': return record(value) && Object.keys(value).length === 2 && Number.isInteger(value.duration_ms)
      && finite(value.duration_ms, 0, 60000) && typeof value.curve === 'string' && Object.hasOwn(curves, value.curve);
    default: return false;
  }
}

/** Validate the whole tree before touching the last committed document. */
export function validateTree(root: unknown, assets: ReadonlySet<string>): asserts root is UiNode {
  const ids = new Set<string>();
  let count = 0;
  function visit(node: unknown, depth: number): void {
    if (!record(node) || Object.keys(node).some(key => !['id', 'kind', 'props', 'children'].includes(key))
      || typeof node.id !== 'string' || !node.id || encoder.encode(node.id).length > 128 || /[\u0000-\u001f\u007f]/.test(node.id)
      || typeof node.kind !== 'string' || !kinds.has(node.kind) || !record(node.props) || !Array.isArray(node.children)
      || depth > 64 || ++count > 5000 || ids.has(node.id)) throw new Error('Invalid Studio web tree');
    ids.add(node.id);
    if (['text', 'image', 'button', 'tag'].includes(node.kind) && node.children.length !== 0) throw new Error('Leaf node has children');
    for (const [key, value] of Object.entries(node.props)) {
      if (!validProperty(node.kind, key, value, assets)) throw new Error(`Invalid ${node.kind} property: ${key}`);
    }
    for (const child of node.children) visit(child, depth + 1);
  }
  visit(root, 1);
}

interface Retained { element: HTMLElement; kind: string; node: UiNode; interactive: boolean }
export class DomRenderer {
  private readonly assets: Set<string>;
  private nodes = new Map<string, Retained>();
  constructor(private readonly container: HTMLElement, assets: readonly string[], private readonly press: (id: string) => void) {
    this.assets = new Set(assets);
    for (const asset of assets) admittedAsset(asset, this.assets);
  }
  commit(root: unknown): void {
    validateTree(root, this.assets);
    const retained = new Map<string, Retained>();
    const doc = this.container.ownerDocument;
    const reconcile = (node: UiNode, visible: boolean): HTMLElement => {
      const previous = this.nodes.get(node.id);
      const element = previous?.kind === node.kind ? previous.element : doc.createElement(node.kind === 'image' ? 'img' : node.kind === 'button' ? 'button' : node.kind === 'text' ? 'p' : 'div');
      const active = visible && node.props.visible !== false;
      const entry: Retained = {element, kind: node.kind, node, interactive: active && node.props.enabled !== false};
      retained.set(node.id, entry);
      element.dataset.nodeId = node.id;
      element.className = `studio-node studio-${node.kind}`;
      element.hidden = node.props.visible === false;
      element.setAttribute('aria-hidden', active ? 'false' : 'true');
      const label = node.props.accessibility_label;
      if (typeof label === 'string') element.setAttribute('aria-label', label); else element.removeAttribute('aria-label');
      this.updateStyle(element, node);
      if (node.kind === 'text' || node.kind === 'tag' || node.kind === 'button') {
        const text = String(node.props[node.kind === 'text' ? 'text' : 'label'] ?? '');
        if (element.textContent !== text) element.textContent = text;
      }
      if (node.kind === 'image') {
        const image = element as HTMLImageElement;
        if (node.props.asset !== undefined) image.setAttribute('src', admittedAsset(node.props.asset, this.assets)); else image.removeAttribute('src');
        image.alt = String(node.props.alt ?? '');
        image.draggable = false;
      }
      if (node.kind === 'button') {
        const button = element as HTMLButtonElement;
        button.type = 'button';
        button.disabled = !entry.interactive;
        button.onclick = () => {
          const current = this.nodes.get(node.id);
          if (current?.element === button && current.interactive) this.press(node.id);
        };
      }
      const children = node.children.map(child => reconcile(child, active));
      let cursor = element.firstElementChild;
      for (const child of children) {
        if (child === cursor) cursor = cursor.nextElementSibling;
        else element.insertBefore(child, cursor);
      }
      while (cursor) {const next = cursor.nextElementSibling; cursor.remove(); cursor = next;}
      return element;
    };
    const element = reconcile(root, true);
    if (this.container.firstElementChild !== element) this.container.replaceChildren(element);
    this.nodes = retained;
  }
  private updateStyle(element: HTMLElement, node: UiNode): void {
    const p = node.props;
    const s = element.style;
    const px = (value: unknown) => typeof value === 'number' ? `${value}px` : '';
    s.padding = px(p.padding); s.gap = px(p.gap);
    s.width = typeof p.width === 'number' ? px(p.width) : p.width === 'full' ? '100%' : '';
    s.height = px(p.height); s.flexGrow = typeof p.flex === 'number' ? String(p.flex) : '';
    s.flexShrink = p.shrink === false ? '0' : '';
    s.backgroundColor = typeof p.background === 'string' ? colors[p.background]! : '';
    s.backgroundImage = p.background_asset ? `url("${admittedAsset(p.background_asset, this.assets)}")` : '';
    s.backgroundSize = p.background_asset ? 'cover' : ''; s.backgroundPosition = p.background_asset ? 'center' : '';
    s.fontSize = px(p.font_size); s.setProperty('--authored-font-size', px(p.font_size));
    element.classList.toggle('studio-large-text', typeof p.font_size === 'number' && p.font_size > 64);
    s.color = typeof p.color === 'string' ? p.color : '';
    if (p.typography_role) element.dataset.typography = String(p.typography_role); else delete element.dataset.typography;
    if (p.variant) element.dataset.variant = String(p.variant); else delete element.dataset.variant;
    const alignment = p.alignment;
    s.alignItems = alignment === 'space_between' ? 'center' : alignment === 'start' ? 'flex-start' : alignment === 'end' ? 'flex-end' : typeof alignment === 'string' ? alignment : '';
    s.justifyContent = alignment === 'space_between' ? 'space-between' : node.kind === 'row' && typeof alignment === 'string' && alignment !== 'stretch' ? alignment.replace('start', 'flex-start').replace('end', 'flex-end') : '';
    const transition = p.transition as {duration_ms: number; curve: string} | undefined;
    s.transition = transition ? `opacity ${transition.duration_ms}ms ${curves[transition.curve]}` : '';
    s.opacity = String(p.opacity ?? 1);
  }
}
