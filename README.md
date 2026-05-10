# glyf-core

[![Crates.io](https://img.shields.io/crates/v/glyf-core.svg)](https://crates.io/crates/glyf-core)
[![docs.rs](https://docs.rs/glyf-core/badge.svg)](https://docs.rs/glyf-core)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

A compact abbreviation parser and HTML/JSX expander inspired by [Emmet](https://emmet.io).

Write a short symbolic abbreviation — get back a full, correctly indented HTML or JSX structure.

```
ul>li.item*3
```
```html
<ul>
	<li class="item"></li>
	<li class="item"></li>
	<li class="item"></li>
</ul>
```

---

## Installation

```toml
[dependencies]
glyf-core = "0.2"
```

---

## Usage

### Expanding an abbreviation

The main entry point is `expand`. It validates the input, parses it, and returns
the rendered string.

```rust
use glyf_core::expand;

let html = expand("div.container>h1+p", None, None).unwrap();
```

```html
<div class="container">
	<h1></h1>
	<p></p>
</div>
```

### Indentation level

Pass `Some(n)` as `base_level` when inserting the expansion inside an
already-indented block. The root element and all its descendants are shifted
by `n` tabs.

```rust
use glyf_core::expand;

let html = expand("div>p", Some(1), None).unwrap();
```

```html

	<div>
		<p></p>
	</div>
```

### JSX mode

By default the library renders standard HTML. Pass a `Config` with
`ParserMode::JSX` to switch to JSX output:

- `class` attributes render as `className`
- `{expr}` prop values are kept unquoted (JSX expressions)
- The `e` abbreviation expands to a JSX fragment `<></>`

```rust
use std::collections::HashMap;
use glyf_core::expand;
use glyf_core::config::{Config, ParserMode};

let config = Config { mode: ParserMode::JSX, snippets: HashMap::new() };
expand("div.container", None, Some(config));
// <div className="container"></div>

let jsx_config = Config { mode: ParserMode::JSX, snippets: HashMap::new() };
expand("e>p", None, Some(jsx_config));
// <><p></p></>
```

### Custom snippets

Provide a `Config` with a `snippets` map to define aliases that expand before
parsing. Custom snippets support the full glyf syntax, including multi-element
expansions containing `>` or `+`.

```rust
use std::collections::HashMap;
use glyf_core::expand;
use glyf_core::config::{Config, ParserMode};

let config = Config {
    mode: ParserMode::HTML,
    snippets: HashMap::from([
        ("mc".to_string(),   "MyComponent".to_string()),
        ("card".to_string(), "div.card>p.card-header+p.card-body".to_string()),
    ]),
};

expand("mc", None, Some(config.clone()));
// <MyComponent></MyComponent>

expand("card", None, Some(config));
// <div class="card">
//     <p class="card-header"></p>
//     <p class="card-body"></p>
// </div>
```

Alternatively, call `Config::init` once at startup and pass `None` for every
subsequent `expand` call:

```rust
use std::collections::HashMap;
use glyf_core::config::{Config, ParserMode};
use glyf_core::expand;

Config::init(
    ParserMode::HTML,
    HashMap::from([("btn".to_string(), "MyButton".to_string())]),
);

expand("btn", None, None);   // <MyButton></MyButton>
expand("div", None, None);   // <div></div>
```

### Working with the AST

If you need the parsed tree rather than a rendered string, use
`parser::parse_input` directly. It returns an `Element` which implements
`Display` for rendering.

```rust
use glyf_core::parser::parse_input;

let element = parse_input("ul>li*3", None).unwrap();

println!("tag:        {:?}", element.identifier);
println!("multiplier: {}", element.multiplier);
println!("has child:  {}", element.node.is_some());
println!("rendered:\n{}", element);
```

### Error handling

`expand` returns `Result<String, GlyfError>` and never panics.
`GlyfError` implements both `Display` and `std::error::Error`, so it
composes naturally with `?` and error-handling crates.

```rust
use glyf_core::{expand, parser::GlyfError};

match expand(abbr, None, None) {
    Ok(html)                           => insert(html),
    Err(GlyfError::UnmatchedBrackets)  => { /* user is still typing */ }
    Err(GlyfError::NoIdentifier)       => { /* empty or operator-only input */ }
}
```

| Variant | Triggered when |
|---|---|
| `GlyfError::UnmatchedBrackets` | Input contains unclosed `(` or `)` |
| `GlyfError::NoIdentifier` | Input has no valid tag name (e.g. bare `">"`) |

---

## Syntax reference

| Syntax | Meaning | Example | Output |
|--------|---------|---------|--------|
| `tag` | Element | `div` | `<div></div>` |
| `tag/` | Self-closing | `br/` | `<br />` |
| `a>b` | Child | `ul>li` | `<ul>` with `<li>` inside |
| `a+b` | Sibling | `div+p` | `<div>` followed by `<p>` |
| `(a+b)*N` | Group × N | `(li)*3` | three `<li>` elements |
| `tag*N` | Repeat | `li*3` | three `<li>` elements |
| `tag.cls` | Class | `div.foo` | `<div class="foo">` |
| `tag#id` | Id | `div#app` | `<div id="app">` |
| `tag:key=val` | Prop | `a:href=url` | `<a href="url">` |
| `tag:key={expr}` | JSX prop | `div:onClick={fn}` | `<div onClick={fn}>` |
| `tag<text` | Text content | `p<Hello` | `<p>Hello</p>` |
| `.cls` / `#id` / `:prop` / `>child` | Implicit `div` | `.foo` | `<div class="foo">` |
| `e` | JSX fragment (JSX mode only) | `e>p` | `<>` with `<p>` inside |

### Operator precedence

When `>` and `+` appear at the same level, the **leftmost operator wins** at the
top of the tree.

```
div>p+span   →  <div><p></p><span></span></div>   (> comes first → child)
div+p>span   →  <div></div><p><span></span></p>   (+ comes first → sibling)
```

---

## Snippets

glyf-core has no built-in snippet table — snippets are entirely user-defined
through `Config`. This keeps the core library small and lets each consumer
(LSP server, CLI tool, etc.) ship its own vocabulary.

A snippet alias maps a short key to any valid glyf abbreviation, including
multi-element structures:

| Key | Value | Expands to |
|-----|-------|-----------|
| `"btn"` | `"MyButton"` | `<MyButton></MyButton>` |
| `"card"` | `"div.card>p.card-body"` | `<div class="card"><p class="card-body">…</p></div>` |
| `"img"` | `"img:src:alt/"` | `<img src alt />` |

When a key is a prefix of the input and is followed by a boundary character
(`.`, `:`, `>`, `+`, etc.), the expansion fires and the tail is appended. For
example, with `"a"` → `"a:href"`:

```
a:id=main  →  a:href:id=main  →  <a href id="main"></a>
```

---

## License

MIT — see [LICENSE](LICENSE).
