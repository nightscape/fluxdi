use std::any::TypeId;
use std::cell::RefCell;
use std::collections::HashSet;

use crate::error::{Error, ErrorKind};
#[cfg(feature = "tracing")]
use crate::observability::EVENT_CIRCULAR_DEPENDENCY;

#[cfg(feature = "tracing")]
use tracing::debug;

thread_local! {
    static RESOLVE_SET: RefCell<HashSet<TypeId>> = RefCell::new(HashSet::new());
}

pub struct ResolveGuard {
    type_id: TypeId,
}

impl ResolveGuard {
    pub fn push(type_id: TypeId) -> Result<Self, Error> {
        RESOLVE_SET.with(|set| {
            let mut set = set.borrow_mut();

            if set.contains(&type_id) {
                #[cfg(feature = "tracing")]
                debug!(
                    event = EVENT_CIRCULAR_DEPENDENCY,
                    type_id = ?type_id,
                    depth = set.len(),
                    "Circular dependency detected during resolve"
                );

                return Err(Error::new(
                    ErrorKind::CircularDependency,
                    format!(
                        "Circular dependency detected while resolving type_id: {:?}",
                        type_id
                    ),
                ));
            }

            set.insert(type_id);
            Ok(Self { type_id })
        })
    }
}

impl Drop for ResolveGuard {
    fn drop(&mut self) {
        RESOLVE_SET.with(|set| {
            set.borrow_mut().remove(&self.type_id);
        });
    }
}
