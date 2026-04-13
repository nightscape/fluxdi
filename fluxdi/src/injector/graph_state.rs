use super::*;

impl Injector {
    fn local_graph_providers(&self) -> HashMap<TypeId, ProviderGraphMeta> {
        #[cfg(not(feature = "thread-safe"))]
        {
            self.inner.graph_providers.borrow().clone()
        }

        #[cfg(all(feature = "thread-safe", not(feature = "lock-free")))]
        {
            self.inner.graph_providers.read().unwrap().clone()
        }

        #[cfg(feature = "lock-free")]
        {
            self.inner
                .graph_providers
                .iter()
                .map(|entry| (*entry.key(), entry.value().clone()))
                .collect()
        }
    }

    fn local_graph_named_providers(&self) -> HashMap<NamedTypeKey, ProviderGraphMeta> {
        #[cfg(not(feature = "thread-safe"))]
        {
            self.inner.graph_named_providers.borrow().clone()
        }

        #[cfg(all(feature = "thread-safe", not(feature = "lock-free")))]
        {
            self.inner.graph_named_providers.read().unwrap().clone()
        }

        #[cfg(feature = "lock-free")]
        {
            self.inner
                .graph_named_providers
                .iter()
                .map(|entry| (entry.key().clone(), entry.value().clone()))
                .collect()
        }
    }

    fn local_graph_set_providers(&self) -> HashMap<TypeId, Vec<ProviderGraphMeta>> {
        #[cfg(not(feature = "thread-safe"))]
        {
            self.inner.graph_set_providers.borrow().clone()
        }

        #[cfg(all(feature = "thread-safe", not(feature = "lock-free")))]
        {
            self.inner.graph_set_providers.read().unwrap().clone()
        }

        #[cfg(feature = "lock-free")]
        {
            self.inner
                .graph_set_providers
                .iter()
                .map(|entry| (*entry.key(), entry.value().clone()))
                .collect()
        }
    }

    #[cfg(feature = "dynamic")]
    fn local_graph_dynamic_providers(
        &self,
    ) -> HashMap<String, crate::graph::DynamicProviderGraphMeta> {
        #[cfg(not(feature = "lock-free"))]
        {
            self.inner.graph_dynamic_providers.read().unwrap().clone()
        }

        #[cfg(feature = "lock-free")]
        {
            self.inner
                .graph_dynamic_providers
                .iter()
                .map(|entry| (entry.key().clone(), entry.value().clone()))
                .collect()
        }
    }

    pub(super) fn collect_graph_state(&self, state: &mut GraphBuildState) {
        if let Some(parent) = &self.inner.parent {
            let parent_injector = Injector {
                inner: parent.clone(),
            };
            parent_injector.collect_graph_state(state);
        }

        for (type_id, meta) in self.local_graph_providers() {
            state.singles.insert(type_id, meta);
        }

        for (key, meta) in self.local_graph_named_providers() {
            state.named.insert(key, meta);
        }

        for (type_id, metas) in self.local_graph_set_providers() {
            state.sets.entry(type_id).or_default().extend(metas);
        }

        #[cfg(feature = "dynamic")]
        for (name, meta) in self.local_graph_dynamic_providers() {
            state.dynamics.insert(name, meta);
        }
    }
}
