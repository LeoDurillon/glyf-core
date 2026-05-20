use std::collections::HashMap;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use glyf_core::{
    config::{Config, ParserMode},
    expand,
};

const INPUTS: &[(&str, &str)] = &[
    ("simple", "div"),
    ("class", "div.foo.bar"),
    ("attrs", "div.foo#main:disabled:role=button"),
    ("child", "div>p"),
    ("chained", "ul>li>a"),
    ("sibling", "div+p+span"),
    ("multiply", "li*10"),
    ("group", "(div>p)*3+span"),
    ("nested", "div>(div>(div>p)+p)+p"),
    ("snippet", "a"),
    ("fragment", "e>label+input:c"),
    (
        "complex",
        "div.fixed.bottom-0.left-0.right-0.top-0.z-20.flex>div.flex.flex-col.items-center>(div.delay>Logo.size-40:fill=white/)+div>Form.bg-surface:action={login}>(div>p>>Connexion+Icon:icon=lock/)+(div>Textfield:label=Email:type=email/+Textfield:label=Password:type=password/)+div>Button:type=submit>>Login",
    ),
];

fn bench_expand(c: &mut Criterion) {
    let mut group = c.benchmark_group("parser/expand");
    let config = Config::new(
        ParserMode::JSX(None),
        HashMap::from([
            ("a".to_string(), "a:href".to_string()),
            ("br".to_string(), "br/".to_string()),
            ("hr".to_string(), "hr/".to_string()),
            ("img".to_string(), "img:src:alt".to_string()),
            ("btn".to_string(), "button".to_string()),
            ("bq".to_string(), "blockquote".to_string()),
            (
                "a:blank".to_string(),
                "a:href=${0}:target=_blank:rel=noopener noreferrer".to_string(),
            ),
            ("input".to_string(), "input/".to_string()),
        ]),
    );

    for (name, input) in INPUTS {
        group.bench_with_input(BenchmarkId::new("expand", name), input, |b, input| {
            b.iter(|| expand(input, None, Some(config.clone())).unwrap());
        });
    }
    group.finish();
}

criterion_group!(benches, bench_expand);
criterion_main!(benches);
