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
glyf-core = "0.1"
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

### Custom snippets

Pass a `HashMap<String, String>` to define aliases that extend or override
the built-in snippet table. Custom snippets follow the same expansion rules
as built-ins, including multi-element expansions.

```rust
use std::collections::HashMap;
use glyf_core::expand;

let snippets = HashMap::from([
    ("mc".to_string(),   "MyComponent".to_string()),
    ("card".to_string(), "div.card>p.card-header+p.card-body".to_string()),
]);

expand("mc", None, Some(&snippets));
// <MyComponent></MyComponent>

expand("card", None, Some(&snippets));
// <div class="card">
//     <p class="card-header"></p>
//     <p class="card-body"></p>
// </div>
```

### Working with the AST

If you need the parsed tree rather than a rendered string, use
`parser::parse_input` directly. It returns an `Element` which implements
`Display` for rendering.

```rust
use glyf_core::parser::parse_input;

let element = parse_input("ul>li*3", None, None).unwrap();

println!("tag:        {:?}", element.identifier);
println!("multiplier: {}", element.multiplier);
println!("has child:  {}", element.node.is_some());
println!("rendered:\n{}", element);
```

### Error handling

`expand` returns `Result<String, GlyfError>` and never panics.
The library is the wrong place to decide whether an error should crash
the program — that decision belongs to the caller.

```rust
use glyf_core::{expand, parser::GlyfError};

match expand(abbr, None, None) {
    Ok(html)                          => insert(html),
    Err(GlyfError::UnmatchedBrackets) => { /* user is still typing */ }
    Err(GlyfError::NoIdentifier)      => { /* empty or operator-only input */ }
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
| `e` | JSX fragment | `e>p` | `<>` with `<p>` inside |

### Operator precedence

When `>` and `+` appear at the same level, the **leftmost operator wins** at the
top of the tree.

```
div>p+span   →  <div><p></p><span></span></div>   (> comes first → child)
div+p>span   →  <div></div><p><span></span></p>   (+ comes first → sibling)
```

---

## Built-in snippets

glyf-core ships **171 built-in aliases** covering the most common HTML elements.
A few examples:

| Alias | Expands to |
|-------|-----------|
| `a` | `<a href></a>` |
| `a:blank` | `<a href target="_blank" rel="noopener noreferrer"></a>` |
| `img` | `<img src alt />` |
| `br` | `<br />` |
| `btn` | `<button></button>` |
| `inp` | `<input type name id />` |
| `e` | `<></>` (JSX fragment) |

Custom snippets passed via `expand` **shadow** built-in aliases when both share
the same key, so you can override any built-in without touching the source.

---

## License

MIT — see [LICENSE](LICENSE).
