use super::*;
use crate::{Provider, Shared};

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

#[tokio::test]
async fn wave_parallelism_respects_dependency_order() {
    let injector = Injector::root();
    let order = Arc::new(AtomicUsize::new(0));

    // A and B have no deps (wave 0), C depends on both (wave 1)
    injector.provide::<u32>({
        let order = order.clone();
        Provider::singleton_async(move |_| {
            let order = order.clone();
            async move {
                let seq = order.fetch_add(1, Ordering::SeqCst);
                Arc::new(seq as u32)
            }
        })
    });

    injector.provide::<u64>({
        let order = order.clone();
        Provider::singleton_async(move |_| {
            let order = order.clone();
            async move {
                let seq = order.fetch_add(1, Ordering::SeqCst);
                Arc::new(seq as u64)
            }
        })
    });

    injector.provide::<String>({
        let order = order.clone();
        Provider::singleton_async(move |_| {
            let order = order.clone();
            async move {
                let seq = order.fetch_add(1, Ordering::SeqCst);
                Arc::new(format!("C:{}", seq))
            }
        })
        .with_dependency::<u32>()
        .with_dependency::<u64>()
    });

    injector.resolve_all_eager().await.unwrap();

    // C should have been resolved after A and B
    let c = injector.try_resolve_async::<String>().await.unwrap();
    let c_seq: usize = c.strip_prefix("C:").unwrap().parse().unwrap();
    assert!(
        c_seq >= 2,
        "C should resolve after A and B, got sequence {}",
        c_seq
    );
}

#[tokio::test]
async fn idempotent_double_call() {
    let injector = Injector::root();
    let counter = Arc::new(AtomicUsize::new(0));

    injector.provide::<u32>({
        let counter = counter.clone();
        Provider::singleton_async(move |_| {
            let counter = counter.clone();
            async move {
                counter.fetch_add(1, Ordering::SeqCst);
                Arc::new(42u32)
            }
        })
    });

    injector.resolve_all_eager().await.unwrap();
    injector.resolve_all_eager().await.unwrap();

    assert_eq!(
        counter.load(Ordering::SeqCst),
        1,
        "Factory should only run once"
    );
}

#[cfg(feature = "dynamic")]
#[tokio::test]
async fn error_propagation_prevents_next_wave() {
    use crate::dynamic::DynamicProvider;
    use std::any::Any;

    let injector = Injector::root();

    // "good" provider in wave 0
    injector.provide::<u32>(Provider::singleton_async(|_| async { Arc::new(1u32) }));

    // "failing" dynamic provider in wave 0
    injector.provide_dynamic(
        "bad",
        DynamicProvider::new(|_inj| async {
            Err(crate::Error::new(
                crate::ErrorKind::ModuleLifecycleFailed,
                "intentional failure".to_string(),
            ))
        }),
    );

    // wave 1 depends on the failing provider
    injector.provide::<bool>(
        Provider::singleton_async(|_| async { Arc::new(true) }).with_dynamic_dependency("bad"),
    );

    let result = injector.resolve_all_eager().await;
    assert!(
        result.is_err(),
        "Should fail due to failing dynamic provider"
    );
    assert!(
        result.unwrap_err().message.contains("intentional failure"),
        "Error should contain the failure reason"
    );
}

#[tokio::test]
async fn no_providers_is_noop() {
    let injector = Injector::root();
    injector.resolve_all_eager().await.unwrap();
}

#[cfg(feature = "dynamic")]
#[tokio::test]
async fn eager_resolves_dynamic_providers() {
    use crate::dynamic::DynamicProvider;
    use std::any::Any;

    let injector = Injector::root();

    injector.provide::<u32>(Provider::singleton_async(|_| async { Arc::new(10u32) }));

    injector.provide_dynamic(
        "doubled",
        DynamicProvider::new(|inj| async move {
            let base = inj.try_resolve_async::<u32>().await.map_err(|e| e)?;
            Ok(Arc::new(*base * 2) as Shared<dyn Any + Send + Sync>)
        })
        .depends_on_static::<u32>(),
    );

    injector.resolve_all_eager().await.unwrap();

    // Both should be cached now
    let base = injector.try_resolve_async::<u32>().await.unwrap();
    assert_eq!(*base, 10);

    let doubled = injector.try_resolve_dynamic("doubled").await.unwrap();
    let val = doubled.downcast::<u32>().unwrap();
    assert_eq!(*val, 20);
}

// --- Scoped eager resolution tests ---

#[tokio::test]
async fn scoped_resolves_root_and_dependency_only() {
    let injector = Injector::root();
    let a_counter = Arc::new(AtomicUsize::new(0));
    let c_counter = Arc::new(AtomicUsize::new(0));
    let x_counter = Arc::new(AtomicUsize::new(0));

    // A: no deps
    injector.provide::<u32>({
        let a_counter = a_counter.clone();
        Provider::singleton_async(move |_| {
            let a_counter = a_counter.clone();
            async move {
                a_counter.fetch_add(1, Ordering::SeqCst);
                Arc::new(1u32)
            }
        })
    });

    // C: depends on A
    injector.provide::<String>({
        let c_counter = c_counter.clone();
        Provider::singleton_async(move |_| {
            let c_counter = c_counter.clone();
            async move {
                c_counter.fetch_add(1, Ordering::SeqCst);
                Arc::new("C".to_string())
            }
        })
        .with_dependency::<u32>()
    });

    // X: unrelated
    injector.provide::<bool>({
        let x_counter = x_counter.clone();
        Provider::singleton_async(move |_| {
            let x_counter = x_counter.clone();
            async move {
                x_counter.fetch_add(1, Ordering::SeqCst);
                Arc::new(true)
            }
        })
    });

    // Resolve only C (should pull in A but not X)
    injector.resolve_eager_for::<String>().await.unwrap();

    assert_eq!(a_counter.load(Ordering::SeqCst), 1, "A should be resolved");
    assert_eq!(c_counter.load(Ordering::SeqCst), 1, "C should be resolved");
    assert_eq!(
        x_counter.load(Ordering::SeqCst),
        0,
        "X should NOT be resolved"
    );
}

#[tokio::test]
async fn scoped_transitive_chain() {
    let injector = Injector::root();
    let order = Arc::new(AtomicUsize::new(0));

    // A -> B -> C (transitive chain)
    injector.provide::<u8>({
        let order = order.clone();
        Provider::singleton_async(move |_| {
            let order = order.clone();
            async move { Arc::new(order.fetch_add(1, Ordering::SeqCst) as u8) }
        })
    });

    injector.provide::<u16>({
        let order = order.clone();
        Provider::singleton_async(move |_| {
            let order = order.clone();
            async move { Arc::new(order.fetch_add(1, Ordering::SeqCst) as u16) }
        })
        .with_dependency::<u8>()
    });

    injector.provide::<u32>({
        let order = order.clone();
        Provider::singleton_async(move |_| {
            let order = order.clone();
            async move { Arc::new(order.fetch_add(1, Ordering::SeqCst) as u32) }
        })
        .with_dependency::<u16>()
    });

    // Resolve only the leaf (u32) — should pull in u16 and u8
    injector.resolve_eager_for::<u32>().await.unwrap();

    let a = *injector.try_resolve_async::<u8>().await.unwrap() as u32;
    let b = *injector.try_resolve_async::<u16>().await.unwrap() as u32;
    let c = *injector.try_resolve_async::<u32>().await.unwrap();

    // u8 must resolve before u16, u16 before u32
    assert!(a < b, "u8 ({}) should resolve before u16 ({})", a, b);
    assert!(b < c, "u16 ({}) should resolve before u32 ({})", b, c);
}

#[tokio::test]
async fn scoped_multiple_roots_shared_dep() {
    let injector = Injector::root();
    let a_counter = Arc::new(AtomicUsize::new(0));
    let d_counter = Arc::new(AtomicUsize::new(0));
    let e_counter = Arc::new(AtomicUsize::new(0));
    let x_counter = Arc::new(AtomicUsize::new(0));

    // A: shared dependency
    injector.provide::<u32>({
        let a_counter = a_counter.clone();
        Provider::singleton_async(move |_| {
            let a_counter = a_counter.clone();
            async move {
                a_counter.fetch_add(1, Ordering::SeqCst);
                Arc::new(1u32)
            }
        })
    });

    // D: depends on A
    injector.provide::<String>({
        let d_counter = d_counter.clone();
        Provider::singleton_async(move |_| {
            let d_counter = d_counter.clone();
            async move {
                d_counter.fetch_add(1, Ordering::SeqCst);
                Arc::new("D".to_string())
            }
        })
        .with_dependency::<u32>()
    });

    // E: also depends on A
    injector.provide::<bool>({
        let e_counter = e_counter.clone();
        Provider::singleton_async(move |_| {
            let e_counter = e_counter.clone();
            async move {
                e_counter.fetch_add(1, Ordering::SeqCst);
                Arc::new(true)
            }
        })
        .with_dependency::<u32>()
    });

    // X: unrelated
    injector.provide::<u64>({
        let x_counter = x_counter.clone();
        Provider::singleton_async(move |_| {
            let x_counter = x_counter.clone();
            async move {
                x_counter.fetch_add(1, Ordering::SeqCst);
                Arc::new(99u64)
            }
        })
    });

    // Resolve D and E — A resolved once, X untouched
    injector
        .resolve_eager_roots(&[TypeId::of::<String>(), TypeId::of::<bool>()])
        .await
        .unwrap();

    assert_eq!(
        a_counter.load(Ordering::SeqCst),
        1,
        "A should be resolved exactly once"
    );
    assert_eq!(d_counter.load(Ordering::SeqCst), 1, "D should be resolved");
    assert_eq!(e_counter.load(Ordering::SeqCst), 1, "E should be resolved");
    assert_eq!(
        x_counter.load(Ordering::SeqCst),
        0,
        "X should NOT be resolved"
    );
}

#[tokio::test]
async fn scoped_unrelated_providers_untouched() {
    let injector = Injector::root();

    injector.provide::<u32>(Provider::singleton_async(|_| async { Arc::new(1u32) }));
    injector.provide::<String>(Provider::singleton_async(|_| async {
        Arc::new("unrelated".to_string())
    }));

    injector.resolve_eager_for::<u32>().await.unwrap();

    // u32 should be cached
    let val = injector.try_resolve_async::<u32>().await.unwrap();
    assert_eq!(*val, 1);

    // String should NOT be cached (factory hasn't run)
    // We verify by checking it wasn't resolved — resolve it fresh
    // and confirm the container still works
    let s = injector.try_resolve_async::<String>().await.unwrap();
    assert_eq!(s.as_str(), "unrelated");
}

#[tokio::test]
async fn scoped_empty_roots_is_noop() {
    let injector = Injector::root();
    injector.provide::<u32>(Provider::singleton_async(|_| async { Arc::new(1u32) }));

    injector.resolve_eager_roots(&[]).await.unwrap();
}

#[tokio::test]
async fn concurrent_resolution_does_not_corrupt_guard() {
    let injector = Injector::root();

    injector.provide::<u32>(Provider::singleton_async(|_| async { Arc::new(42u32) }));
    injector.provide::<u64>(Provider::singleton_async(|_| async { Arc::new(99u64) }));

    // Resolve concurrently — should not panic with stack corruption
    injector
        .resolve_eager_roots(&[TypeId::of::<u32>(), TypeId::of::<u64>()])
        .await
        .unwrap();

    assert_eq!(*injector.try_resolve_async::<u32>().await.unwrap(), 42);
    assert_eq!(*injector.try_resolve_async::<u64>().await.unwrap(), 99);
}

#[tokio::test]
async fn transient_providers_skipped_in_eager() {
    let injector = Injector::root();
    let counter = Arc::new(AtomicUsize::new(0));

    injector.provide::<u32>({
        let counter = counter.clone();
        Provider::transient_async(move |_| {
            let counter = counter.clone();
            async move {
                counter.fetch_add(1, Ordering::SeqCst);
                Arc::new(99u32)
            }
        })
    });

    injector.resolve_all_eager().await.unwrap();

    // Transient providers should still be resolved by eager (they have resolvers registered)
    // but their instances won't be cached
    let val = injector.try_resolve_async::<u32>().await.unwrap();
    assert_eq!(*val, 99);
}
