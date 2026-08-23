export * from "./components/layout";
export * from "./components/display";
export * from "./components/interaction";
export * from "./components/overlay";
export * from "./components/navigation";
export * from "./components/data";
export * from "./components/advanced";

/** AssemblyScript UI Widget Node class with fluent builder methods. */
export class Widget {
  id: string;
  kind: string;
  props: Map<string, string>;
  children: Array<Widget>;

  constructor(id: string, kind: string) {
    if (id.length == 0) throw new Error("widget id must not be empty");
    this.id = id;
    this.kind = kind;
    this.props = new Map<string, string>();
    this.children = new Array<Widget>();
  }

  /** Append a single child widget. */
  child(childNode: Widget | null): Widget {
    if (childNode !== null) {
      this.children.push(childNode);
    }
    return this;
  }

  /** Append multiple child widgets. */
  addChildren(childNodes: Array<Widget>): Widget {
    for (let i: i32 = 0; i < childNodes.length; i++) {
      this.children.push(childNodes[i]);
    }
    return this;
  }

  /** Set a string property value. */
  prop(key: string, value: string): Widget {
    this.props.set(key, value);
    return this;
  }

  /** Set element gap spacing in pixels. */
  gap(amount: i32): Widget {
    this.props.set("gap", amount.toString());
    return this;
  }

  /** Set element padding in pixels. */
  padding(amount: i32): Widget {
    this.props.set("padding", amount.toString());
    return this;
  }

  /** Set flex grow factor. */
  flex(flexValue: f64): Widget {
    this.props.set("flex", flexValue.toString());
    return this;
  }

  /** Set layout alignment. */
  alignment(align: string): Widget {
    this.props.set("alignment", align);
    return this;
  }

  /** Set text label. */
  label(lbl: string): Widget {
    this.props.set("label", lbl);
    return this;
  }

  /** Set text or option value. */
  value(val: string): Widget {
    this.props.set("value", val);
    return this;
  }

  /** Set enabled boolean state. */
  enabled(en: bool): Widget {
    this.props.set("enabled", en ? "true" : "false");
    return this;
  }

  /** Set selection options array. */
  options(opts: Array<string>): Widget {
    let json: string = "[";
    for (let i: i32 = 0; i < opts.length; i++) {
      if (i > 0) json += ",";
      json += '"' + opts[i] + '"';
    }
    json += "]";
    this.props.set("options", json);
    return this;
  }

  /** Set placeholder text. */
  placeholder(ph: string): Widget {
    this.props.set("placeholder", ph);
    return this;
  }

  /** Set title text. */
  title(t: string): Widget {
    this.props.set("title", t);
    return this;
  }

  /** Set open/expanded state. */
  open(op: bool): Widget {
    this.props.set("open", op ? "true" : "false");
    return this;
  }

  /** Set pressed callback name or token. */
  onPressed(token: string): Widget {
    this.props.set("on_pressed", token);
    return this;
  }

  /** Set changed callback name or token. */
  onChanged(token: string): Widget {
    this.props.set("on_changed", token);
    return this;
  }

  /** Set value changed callback name or token. */
  onValueChanged(token: string): Widget {
    this.props.set("on_changed", token);
    return this;
  }

  /** Set date changed callback name or token. */
  onDateChanged(token: string): Widget {
    this.props.set("on_changed", token);
    return this;
  }

  /** Set navigation callback name or token. */
  onNavigate(token: string): Widget {
    this.props.set("on_navigate", token);
    return this;
  }

  /** Set secret input ready callback name or token. */
  onReady(token: string): Widget {
    this.props.set("on_ready", token);
    return this;
  }
}

/** Create a stable protocol widget node. */
export function widget(id: string, kind: string): Widget {
  return new Widget(id, kind);
}
