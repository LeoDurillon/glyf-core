use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use glyf_core::compress;

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

fn bench_compress(c: &mut Criterion) {
    let mut group = c.benchmark_group("compress");
    for (name, html) in INPUTS {
        group.bench_with_input(BenchmarkId::new("compress", name), html, |b, html| {
            b.iter(|| compress(html).unwrap());
        });
    }
    group.finish();
}

criterion_group!(benches, bench_compress);
criterion_main!(benches);
