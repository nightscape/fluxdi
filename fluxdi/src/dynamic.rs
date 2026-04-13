//! Dynamic providers for runtime-registered services.
//!
//! Dynamic providers are string-keyed, type-erased providers that participate
//! in the same dependency graph as typed providers. They are useful for plugin
//! architectures where not all services are known at compile time.

use std::any::{Any, TypeId};
use std::future::Future;
use std::pin::Pin;

use crate::error::Error;
use crate::injector::Injector;
use crate::runtime::Shared;
use crate::scope::Scope;

/// A type-erased async factory for dynamic providers.
pub type DynamicFactory = Box<
    dyn Fn(
            Injector,
        ) -> Pin<
            Box<dyn Future<Output = Result<Shared<dyn Any + Send + Sync>, Error>> + Send + 'static>,
        > + Send
        + Sync
        + 'static,
>;

/// Declares a dependency of a dynamic provider on either a typed or another dynamic provider.
pub enum DynamicDependency {
    /// Depends on a typed provider (bridges into the static graph).
    Static {
        type_id: TypeId,
        type_name: &'static str,
    },
    /// Depends on another dynamic provider by name.
    Named(String),
}

/// A runtime-registered, string-keyed provider that produces type-erased instances.
///
/// Dynamic providers participate in the dependency graph alongside typed providers
/// and can be eagerly resolved via [`Injector::resolve_all_eager()`].
pub struct DynamicProvider {
    /// The lifecycle scope of this provider.
    pub scope: Scope,
    /// The async factory function.
    pub factory: DynamicFactory,
    /// Declared dependencies for graph validation and eager resolution ordering.
    pub dependencies: Vec<DynamicDependency>,
}

impl DynamicProvider {
    /// Creates a new dynamic provider with the given async factory.
    ///
    /// Defaults to `Scope::Module` (singleton within the module).
    pub fn new<F, Fut>(factory: F) -> Self
    where
        F: Fn(Injector) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<Shared<dyn Any + Send + Sync>, Error>> + Send + 'static,
    {
        Self {
            scope: Scope::Module,
            factory: Box::new(move |inj| Box::pin(factory(inj))),
            dependencies: Vec::new(),
        }
    }

    /// Sets the lifecycle scope.
    pub fn with_scope(mut self, scope: Scope) -> Self {
        self.scope = scope;
        self
    }

    /// Declares a dependency on a typed (static) provider.
    pub fn depends_on_static<T: ?Sized + 'static>(mut self) -> Self {
        self.dependencies.push(DynamicDependency::Static {
            type_id: TypeId::of::<T>(),
            type_name: std::any::type_name::<T>(),
        });
        self
    }

    /// Declares a dependency on another dynamic provider by name.
    pub fn depends_on_named(mut self, name: impl Into<String>) -> Self {
        self.dependencies
            .push(DynamicDependency::Named(name.into()));
        self
    }
}
