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
glyf-core = "0.3.2"
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
use glyf_core::expand;
use glyf_core::config::{Config, ParserMode};
use std::collections::HashMap;

let config = Config::new(ParserMode::JSX, HashMap::new());
expand("div.container", None, Some(config));
// <div className="container"></div>

let jsx_config = Config::new(ParserMode::JSX, HashMap::new());
expand("e>p", None, Some(jsx_config));
// <><p></p></>
```

### Custom snippets

Provide a `Config` with a `snippets` map to define aliases that expand before
parsing. Custom snippets support the full glyf syntax, including multi-element
expansions containing `>` or `+`.

```rust
use glyf_core::expand;
use glyf_core::config::{Config, ParserMode};
use std::collections::HashMap;

let config = Config::new(
    ParserMode::HTML,
    HashMap::from([
        ("mc".to_string(),   "MyComponent".to_string()),
        ("card".to_string(), "div.card>p.card-header+p.card-body".to_string()),
    ]),
);

expand("mc", None, Some(config.clone()));
// <MyComponent></MyComponent>

expand("card", None, Some(config));
// <div class="card">
//     <p class="card-header"></p>
//     <p class="card-body"></p>
// </div>
```


### Compressing HTML to an abbreviation

`compress` is the inverse of `expand` — it converts HTML markup back to the
shortest Glyf abbreviation that would regenerate equivalent output.

```rust
use glyf_core::compress;

assert_eq!(compress("<div class=\"card\"><p></p></div>").unwrap(), "div.card>p");
assert_eq!(compress("<div></div><span></span>").unwrap(), "div+span");
assert_eq!(compress("<div><p></p></div><span></span>").unwrap(), "(div>p)+span");
```

### Working with the AST

Use `expand_to_tree` or `compress_to_tree` when you need the parsed
[`Element`](https://docs.rs/glyf-core/latest/glyf_core/parser/struct.Element.html)
tree rather than a string. `Element` implements `Display` for rendering back
to HTML and `to_glyf()` for serialising back to an abbreviation.

```rust
use glyf_core::expand_to_tree;

let element = expand_to_tree("ul>li*3", None, None).unwrap();

println!("tag:        {:?}", element.identifier);
println!("multiplier: {}", element.multiplier);
println!("has child:  {}", element.node.is_some());
println!("rendered:\n{}", element);
```

```rust
use glyf_core::compress_to_tree;
use glyf_core::parser::Node;

let el = compress_to_tree("<ul><li></li></ul>", None).unwrap();
assert_eq!(el.identifier.as_deref(), Some("ul"));
assert!(matches!(*el.node.unwrap(), Node::Children(_)));
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
    Err(GlyfError::EmptyInput)         => { /* blank string */ }
    Err(GlyfError::MalformedHtml(msg)) => { /* invalid HTML passed to compress */ }
}
```

| Variant | Triggered when |
|---|---|
| `GlyfError::UnmatchedBrackets` | Input contains unclosed `(` or `)` |
| `GlyfError::NoIdentifier` | Input has no valid tag name (e.g. bare `">"`) |
| `GlyfError::EmptyInput` | Input is an empty string |
| `GlyfError::MalformedHtml` | HTML passed to `compress` / `compress_to_tree` is not parseable |

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
| `tag>>text` | Text content | `p>>Hello` | `<p>Hello</p>` |
| `tag#{expr}` | JSX dynamic id | `div#{myId}` | `<div id={myId}>` |
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
