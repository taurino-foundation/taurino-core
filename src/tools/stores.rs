use crate::{
  tools::{ArcMut, arc_mut},
  unsafe_impl_sync_send,
};
use anyhow::Result;
use parking_lot::RwLock;
use std::{
  collections::{HashMap, HashSet},
  ops::Deref,
  path::PathBuf,
  sync::{Arc, Mutex, MutexGuard},
};
use tao::event::DeviceId;
use wry::WebContext as WryWebContext;

#[derive(Debug)]
pub struct WebContext {
  pub inner: WryWebContext,
  pub referenced_by_webviews: HashSet<String>,
  // on Linux the custom protocols are associated with the context
  // and you cannot register a URI scheme more than once
  pub registered_custom_protocols: HashSet<String>,
}

// crates/taurino-core/src/lib.rs

#[derive(Clone)]
pub struct WebContextStore(ArcMut<HashMap<Option<PathBuf>, WebContext>>);

unsafe_impl_sync_send!(WebContextStore);

impl Deref for WebContextStore {
  type Target = Mutex<HashMap<Option<PathBuf>, WebContext>>;
  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl WebContextStore {
  pub fn new() -> Self {
    Self(arc_mut(HashMap::new()))
  }
  pub fn lock(&self) -> std::sync::LockResult<MutexGuard<'_, HashMap<Option<PathBuf>, WebContext>>> {
    self.0.lock()
  }
}

struct DeviceRegistryInner {
  next_id: u32,
  by_device: HashMap<DeviceId, u32>,
  by_id: HashMap<u32, DeviceId>,
}

#[derive(Clone)]
pub struct DeviceRegistry {
  inner: Arc<RwLock<DeviceRegistryInner>>,
}

impl DeviceRegistry {
  pub fn new() -> Result<ArcMut<Self>> {
    Ok(arc_mut(Self {
      inner: Arc::new(RwLock::new(DeviceRegistryInner {
        next_id: 1,
        by_device: HashMap::new(),
        by_id: HashMap::new(),
      })),
    }))
  }

  fn next_available_id(inner: &mut DeviceRegistryInner) -> u32 {
    loop {
      let id = inner.next_id;

      inner.next_id = inner.next_id.checked_add(1).unwrap_or(1);

      if !inner.by_id.contains_key(&id) {
        return id;
      }
    }
  }

  /// Returns the existing runtime id for this device.
  /// If the device is not registered yet, creates a new id first.
  pub fn get_or_create_id(&self, device_id: DeviceId) -> u32 {
    let mut inner = self.inner.write();

    if let Some(id) = inner.by_device.get(&device_id) {
      return *id;
    }

    let id = Self::next_available_id(&mut inner);

    inner.by_device.insert(device_id, id);
    inner.by_id.insert(id, device_id);

    id
  }

  pub fn get_id(&self, device_id: &DeviceId) -> Option<u32> {
    self.inner.read().by_device.get(device_id).copied()
  }

  pub fn get_device(&self, id: u32) -> Option<DeviceId> {
    self.inner.read().by_id.get(&id).copied()
  }

  pub fn remove_by_device(&self, device_id: &DeviceId) -> Option<u32> {
    let mut inner = self.inner.write();

    let id = inner.by_device.remove(device_id)?;
    inner.by_id.remove(&id);

    Some(id)
  }

  pub fn remove_by_id(&self, id: u32) -> Option<DeviceId> {
    let mut inner = self.inner.write();

    let device_id = inner.by_id.remove(&id)?;
    inner.by_device.remove(&device_id);

    Some(device_id)
  }

  pub fn clear(&self) {
    let mut inner = self.inner.write();

    inner.by_device.clear();
    inner.by_id.clear();
    inner.next_id = 1;
  }
}
