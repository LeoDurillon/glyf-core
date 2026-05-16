use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use glyf_core::{config::Config, parser::parse_html};

const INPUTS: &[(&str, &str)] = &[
    ("simple", "<div></div>"),
    ("class", "<div class=\"foo bar\"></div>"),
    ("attrs", "<div id=\"main\" class=\"foo\"></div>"),
    ("child", "<div><p></p></div>"),
    ("chained", "<ul><li><a></a></li></ul>"),
    ("sibling", "<div></div><p></p><span></span>"),
    ("self_closing", "<br />"),
    ("child_sibling", "<div><p></p></div><span></span>"),
    ("nested", "<div><div><div><p></p></div></div></div>"),
    ("text", "<p>Hello world</p>"),
    (
        "complex",
        "<div class=\"container\"><header><nav><a href=\"url\">Link</a></nav></header><main><section><h1>Title</h1><p>Content</p></section></main><footer></footer></div>",
    ),
];

/// Benchmarks only `Element::to_glyf` — the Glyf-string generation step
/// without the HTML parsing overhead.
fn bench_element_to_glyf(c: &mut Criterion) {
    let config = Config::default();
    let mut group = c.benchmark_group("element/to_glyf");

    for (name, html) in INPUTS {
        // Build the element once outside the timed loop.
        let Ok(element) = parse_html(html, None, &config) else {
            continue;
        };
        group.bench_with_input(BenchmarkId::new("to_glyf", name), &element, |b, el| {
            b.iter(|| el.to_glyf())
        });
    }
    group.finish();
}

criterion_group!(benches, bench_element_to_glyf);
criterion_main!(benches);
