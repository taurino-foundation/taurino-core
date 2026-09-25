use std::{
    any::{Any, TypeId, type_name},
    borrow::Cow,
    collections::BTreeMap,
    sync::Arc,
};

use anyhow::{Result, anyhow};

/// Resources are Rust objects that are stored in [ResourceTable] and managed by tauri.
///
/// They are identified in JS by a numeric ID (the resource ID, or rid).
/// Resources can be created in commands. Resources can also be retrieved in commands by
/// their rid. Resources are thread-safe.
///
/// Resources are reference counted in Rust. This means that they can be
/// cloned and passed around. When the last reference is dropped, the resource
/// is automatically closed. As long as the resource exists in the resource
/// table, the reference count is at least 1.
pub trait Resource: Any + 'static + Send + Sync {
    /// Returns a string representation of the resource. The default implementation
    /// returns the Rust type name, but specific resource types may override this
    /// trait method.
    fn name(&self) -> Cow<'_, str> {
        type_name::<Self>().into()
    }

    /// Resources may implement the `close()` trait method if they need to do
    /// resource-specific clean-ups, such as cancelling pending futures, after a
    /// resource has been removed from the resource table.
    fn close(self: Arc<Self>) {}
}

impl dyn Resource {
    #[inline(always)]
    fn is<T: Resource>(&self) -> bool {
        self.type_id() == TypeId::of::<T>()
    }

    #[inline(always)]
    pub(crate) fn downcast_arc<T: Resource>(self: &Arc<Self>) -> Option<&Arc<T>> {
        if self.is::<T>() {
            // A resource is stored as `Arc<T>` in a BTreeMap
            // and is safe to cast to `Arc<T>` because of the runtime
            // check done in `self.is::<T>()`.
            let ptr = self as *const Arc<_> as *const Arc<T>;
            Some(unsafe { &*ptr })
        } else {
            None
        }
    }
}

/// A `ResourceId` is an integer value referencing a resource. It could be
/// considered to be the tauri equivalent of a file descriptor in POSIX-like
/// operating systems.
pub type ResourceId = u32;

/// Map-like data structure storing Tauri's resources.
///
/// Provides basic methods for element access. A resource can be of any type.
/// Different types of resources can be stored in the same map and provided
/// with a name for description.
///
/// Each resource is identified through a resource ID (`rid`), which acts as
/// the key in the map.
#[derive(Default, Clone)]
pub struct ResourceTable {
    index: BTreeMap<ResourceId, Arc<dyn Resource>>,
}

impl ResourceTable {
    fn new_random_rid() -> u32 {
        let mut bytes = [0_u8; 4];

        getrandom::fill(&mut bytes).expect("failed to get random bytes");

        u32::from_ne_bytes(bytes)
    }

    /// Inserts a resource into the resource table, which takes ownership of it.
    ///
    /// The resource type is erased at runtime and must be statically known
    /// when retrieving it through `get()`.
    ///
    /// Returns a unique resource ID, which acts as a key for this resource.
    pub fn add<T: Resource>(&mut self, resource: T) -> ResourceId {
        self.add_arc(Arc::new(resource))
    }

    /// Inserts an `Arc`-wrapped resource into the resource table.
    ///
    /// The resource type is erased at runtime and must be statically known
    /// when retrieving it through `get()`.
    ///
    /// Returns a unique resource ID, which acts as a key for this resource.
    pub fn add_arc<T: Resource>(&mut self, resource: Arc<T>) -> ResourceId {
        let resource = resource as Arc<dyn Resource>;
        self.add_arc_dyn(resource)
    }

    /// Inserts an `Arc`-wrapped dynamically typed resource into the resource table.
    ///
    /// Returns a unique resource ID, which acts as a key for this resource.
    pub fn add_arc_dyn(&mut self, resource: Arc<dyn Resource>) -> ResourceId {
        let mut rid = Self::new_random_rid();

        while self.index.contains_key(&rid) {
            rid = Self::new_random_rid();
        }

        let removed_resource = self.index.insert(rid, resource);

        assert!(removed_resource.is_none());

        rid
    }

    /// Returns `true` if any resource with the given `rid` exists.
    pub fn has(&self, rid: ResourceId) -> bool {
        self.index.contains_key(&rid)
    }

    /// Returns a reference-counted pointer to the resource of type `T`
    /// associated with `rid`.
    ///
    /// Returns an error if the resource does not exist or if the resource
    /// exists but has a different type.
    pub fn get<T: Resource>(&self, rid: ResourceId) -> Result<Arc<T>> {
        let resource = self
            .index
            .get(&rid)
            .ok_or_else(|| anyhow!("resource with id {rid} does not exist"))?;

        resource.downcast_arc::<T>().cloned().ok_or_else(|| {
            anyhow!(
                "resource with id {rid} has the wrong type; expected {}",
                type_name::<T>()
            )
        })
    }

    /// Returns a reference-counted pointer to the resource associated with `rid`.
    ///
    /// Returns an error if the resource does not exist.
    pub fn get_any(&self, rid: ResourceId) -> Result<Arc<dyn Resource>> {
        self.index
            .get(&rid)
            .cloned()
            .ok_or_else(|| anyhow!("resource with id {rid} does not exist"))
    }

    /// Replaces an existing resource with a new resource.
    ///
    /// Panics if the resource does not exist.
    pub fn replace<T: Resource>(&mut self, rid: ResourceId, resource: T) {
        let result = self
            .index
            .insert(rid, Arc::new(resource) as Arc<dyn Resource>);

        assert!(
            result.is_some(),
            "cannot replace resource with id {rid}: resource does not exist"
        );
    }

    /// Removes a resource of type `T` from the resource table and returns it.
    ///
    /// If a resource with the given `rid` exists but its type does not match
    /// `T`, it is not removed.
    ///
    /// The resource's `close()` method is not called.
    ///
    /// Note that the returned `Arc<T>` may still be referenced elsewhere.
    pub fn take<T: Resource>(&mut self, rid: ResourceId) -> Result<Arc<T>> {
        // Perform the type check before removing the resource so that a resource
        // with the wrong type remains in the table.
        self.get::<T>(rid)?;

        let resource = self
            .index
            .remove(&rid)
            .ok_or_else(|| anyhow!("resource with id {rid} does not exist"))?;

        resource.downcast_arc::<T>().cloned().ok_or_else(|| {
            anyhow!(
                "resource with id {rid} has the wrong type; expected {}",
                type_name::<T>()
            )
        })
    }

    /// Removes a resource from the resource table and returns it.
    ///
    /// The resource's `close()` method is not called.
    ///
    /// Note that the returned `Arc<dyn Resource>` may still be referenced elsewhere.
    pub fn take_any(&mut self, rid: ResourceId) -> Result<Arc<dyn Resource>> {
        self.index
            .remove(&rid)
            .ok_or_else(|| anyhow!("resource with id {rid} does not exist"))
    }

    /// Returns an iterator yielding `(id, name)` for every currently stored
    /// resource.
    pub fn names(&self) -> impl Iterator<Item = (ResourceId, Cow<'_, str>)> {
        self.index
            .iter()
            .map(|(&id, resource)| (id, resource.name()))
    }

    /// Removes the resource with the given `rid` from the resource table and
    /// invokes its `close()` implementation.
    ///
    /// Other `Arc` references may continue to keep the resource alive after
    /// removal from the table.
    pub fn close(&mut self, rid: ResourceId) -> Result<()> {
        let resource = self
            .index
            .remove(&rid)
            .ok_or_else(|| anyhow!("resource with id {rid} does not exist"))?;

        resource.close();

        Ok(())
    }

    /// Removes and frees all resources stored in the table.
    ///
    /// The resources' `close()` methods are not called.
    pub(crate) fn clear(&mut self) {
        self.index.clear();
    }
}
