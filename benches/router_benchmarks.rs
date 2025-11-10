// Performance benchmarks for Sentium Bridge Protocol
use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use sentium_bridge::core::router::{IntentTranslator, Intent, RoutingEngine};

fn bench_intent_translation(c: &mut Criterion) {
    let translator = IntentTranslator::new();
    
    let intent = Intent {
        id: "bench-1".to_string(),
        from_chain: "ethereum".to_string(),
        to_chain: "polkadot".to_string(),
        action: "transfer".to_string(),
        params: vec![],
        context: vec![],
    };
    
    c.bench_function("translate_intent", |b| {
        b.iter(|| {
            translator.translate(black_box(&intent))
        })
    });
}

fn bench_route_finding(c: &mut Criterion) {
    let mut engine = RoutingEngine::new();
    
    let intent = Intent {
        id: "bench-route-1".to_string(),
        from_chain: "ethereum".to_string(),
        to_chain: "polkadot".to_string(),
        action: "transfer".to_string(),
        params: vec![],
        context: vec![],
    };
    
    c.bench_function("find_route", |b| {
        b.iter(|| {
            engine.find_route(black_box(&intent))
        })
    });
}

fn bench_all_routes(c: &mut Criterion) {
    let engine = RoutingEngine::new();
    
    let mut group = c.benchmark_group("all_routes");
    
    for max_hops in [1, 2, 3].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(max_hops),
            max_hops,
            |b, &max_hops| {
                b.iter(|| {
                    engine.get_all_routes(
                        black_box("ethereum"),
                        black_box("polkadot"),
                        black_box(max_hops),
                    )
                })
            },
        );
    }
    
    group.finish();
}

fn bench_context_operations(c: &mut Criterion) {
    use sentium_bridge::core::context::{SemanticContext, UserPreferences, RiskLevel};
    
    let prefs = UserPreferences {
        slippage_tolerance: 0.01,
        max_gas_price: 100,
        min_confirmations: 6,
        preferred_routes: vec![],
        risk_tolerance: RiskLevel::Medium,
    };
    
    c.bench_function("create_context", |b| {
        b.iter(|| {
            SemanticContext::new(
                black_box("intent-1".to_string()),
                black_box("ethereum".to_string()),
                black_box("polkadot".to_string()),
                black_box(prefs.clone()),
            )
        })
    });
    
    let context = SemanticContext::new(
        "intent-1".to_string(),
        "ethereum".to_string(),
        "polkadot".to_string(),
        prefs,
    );
    
    c.bench_function("verify_integrity", |b| {
        b.iter(|| {
            black_box(&context).verify_integrity()
        })
    });
}

criterion_group!(
    benches,
    bench_intent_translation,
    bench_route_finding,
    bench_all_routes,
    bench_context_operations
);
criterion_main!(benches);
