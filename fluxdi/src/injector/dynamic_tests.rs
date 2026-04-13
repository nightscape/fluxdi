use super::*;
use crate::dynamic::DynamicProvider;
use crate::{Provider, Shared};

use std::any::Any;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use futures::executor::block_on;

#[test]
fn dynamic_provider_resolves() {
    let injector = Injector::root();
    injector.provide_dynamic(
        "greeting",
        DynamicProvider::new(|_inj| async {
            Ok(Arc::new(String::from("hello")) as Shared<dyn Any + Send + Sync>)
        }),
    );

    let value = block_on(injector.try_resolve_dynamic("greeting")).unwrap();
    let greeting = value.downcast::<String>().unwrap();
    assert_eq!(greeting.as_str(), "hello");
}

#[test]
fn dynamic_provider_not_found() {
    let injector = Injector::root();
    let result = block_on(injector.try_resolve_dynamic("missing"));
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().kind,
        crate::ErrorKind::DynamicProviderNotFound
    );
}

#[test]
fn dynamic_provider_is_cached() {
    let injector = Injector::root();
    let counter = Arc::new(AtomicUsize::new(0));

    injector.provide_dynamic("counter", {
        let counter = counter.clone();
        DynamicProvider::new(move |_inj| {
            let counter = counter.clone();
            async move {
                let val = counter.fetch_add(1, Ordering::SeqCst);
                Ok(Arc::new(val) as Shared<dyn Any + Send + Sync>)
            }
        })
    });

    let first = block_on(injector.try_resolve_dynamic("counter")).unwrap();
    let second = block_on(injector.try_resolve_dynamic("counter")).unwrap();
    assert!(Arc::ptr_eq(&first, &second));
    assert_eq!(counter.load(Ordering::SeqCst), 1);
}

#[test]
fn dynamic_transient_not_cached() {
    let injector = Injector::root();
    let counter = Arc::new(AtomicUsize::new(0));

    injector.provide_dynamic("counter", {
        let counter = counter.clone();
        DynamicProvider::new(move |_inj| {
            let counter = counter.clone();
            async move {
                let val = counter.fetch_add(1, Ordering::SeqCst);
                Ok(Arc::new(val) as Shared<dyn Any + Send + Sync>)
            }
        })
        .with_scope(crate::Scope::Transient)
    });

    let first = block_on(injector.try_resolve_dynamic("counter")).unwrap();
    let second = block_on(injector.try_resolve_dynamic("counter")).unwrap();
    assert!(!Arc::ptr_eq(&first, &second));
    assert_eq!(counter.load(Ordering::SeqCst), 2);
}

#[test]
fn dynamic_depends_on_static() {
    let injector = Injector::root();
    injector.provide::<String>(Provider::singleton(|_| Arc::new("db_url".to_string())));

    injector.provide_dynamic(
        "db_view",
        DynamicProvider::new(|inj| async move {
            let url: Arc<String> = inj.try_resolve_async::<String>().await.map_err(|e| e)?;
            let view = format!("view({})", url);
            Ok(Arc::new(view) as Shared<dyn Any + Send + Sync>)
        })
        .depends_on_static::<String>(),
    );

    let report = injector.validate_graph();
    assert!(
        report.is_valid(),
        "Graph should be valid: {}",
        report.summary()
    );

    let value = block_on(injector.try_resolve_dynamic("db_view")).unwrap();
    let view = value.downcast::<String>().unwrap();
    assert_eq!(view.as_str(), "view(db_url)");
}

#[test]
fn static_depends_on_dynamic() {
    let injector = Injector::root();

    injector.provide_dynamic(
        "config",
        DynamicProvider::new(|_inj| async { Ok(Arc::new(42u32) as Shared<dyn Any + Send + Sync>) }),
    );

    injector.provide::<String>(
        Provider::singleton(|_| Arc::new("service".to_string())).with_dynamic_dependency("config"),
    );

    let report = injector.validate_graph();
    assert!(
        report.is_valid(),
        "Graph should be valid: {}",
        report.summary()
    );
}

#[test]
fn dynamic_depends_on_dynamic() {
    let injector = Injector::root();

    injector.provide_dynamic(
        "base",
        DynamicProvider::new(|_inj| async { Ok(Arc::new(10u32) as Shared<dyn Any + Send + Sync>) }),
    );

    injector.provide_dynamic(
        "derived",
        DynamicProvider::new(|inj| async move {
            let base = inj.try_resolve_dynamic("base").await?;
            let base_val = *base.downcast::<u32>().unwrap();
            Ok(Arc::new(base_val * 2) as Shared<dyn Any + Send + Sync>)
        })
        .depends_on_named("base"),
    );

    let report = injector.validate_graph();
    assert!(
        report.is_valid(),
        "Graph should be valid: {}",
        report.summary()
    );

    let value = block_on(injector.try_resolve_dynamic("derived")).unwrap();
    let result = value.downcast::<u32>().unwrap();
    assert_eq!(*result, 20);
}

#[test]
fn mixed_cycle_detection() {
    let injector = Injector::root();

    // static A depends on dynamic "B"
    injector.provide::<u32>(Provider::singleton(|_| Arc::new(1u32)).with_dynamic_dependency("B"));

    // dynamic "B" depends on static A
    injector.provide_dynamic(
        "B",
        DynamicProvider::new(|_inj| async { Ok(Arc::new(2u32) as Shared<dyn Any + Send + Sync>) })
            .depends_on_static::<u32>(),
    );

    let report = injector.validate_graph();
    assert!(!report.is_valid(), "Cycle should be detected");
    assert!(
        report.summary().contains("circular"),
        "Should mention circular: {}",
        report.summary()
    );
}

#[test]
fn graph_visualization_includes_dynamic() {
    let injector = Injector::root();

    injector.provide::<String>(Provider::singleton(|_| Arc::new("hello".to_string())));
    injector.provide_dynamic(
        "my_view",
        DynamicProvider::new(|_inj| async { Ok(Arc::new(1u32) as Shared<dyn Any + Send + Sync>) })
            .depends_on_static::<String>(),
    );

    let graph = injector.dependency_graph();
    let dot = graph.to_dot();
    let mermaid = graph.to_mermaid();

    assert!(
        dot.contains("my_view"),
        "DOT should contain dynamic node: {}",
        dot
    );
    assert!(
        dot.contains("dynamic:my_view"),
        "DOT should contain dynamic binding label: {}",
        dot
    );
    assert!(
        mermaid.contains("my_view"),
        "Mermaid should contain dynamic node: {}",
        mermaid
    );
}

#[test]
fn duplicate_dynamic_provider_rejected() {
    let injector = Injector::root();

    injector.provide_dynamic(
        "dup",
        DynamicProvider::new(|_inj| async { Ok(Arc::new(1u32) as Shared<dyn Any + Send + Sync>) }),
    );

    let result = injector.try_provide_dynamic(
        "dup",
        DynamicProvider::new(|_inj| async { Ok(Arc::new(2u32) as Shared<dyn Any + Send + Sync>) }),
    );
    assert!(result.is_err());
}
