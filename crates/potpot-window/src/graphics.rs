//! The graphics device the windows of one plugin draw with.

use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};
use std::sync::{Arc, LazyLock, Mutex, OnceLock, PoisonError, TryLockError, Weak};

use baseview::HandlerError;
use tracing::warn;

/// A graphics device and its queue.
///
/// Every window that one loaded copy of this crate has open draws with the same one, and each
/// plugin binary has a copy of its own. Each window holds the device, and so does the
/// [`Shared`](crate::Shared) a window was last opened with. It is let go of when the last of
/// them is.
#[derive(Debug)]
pub(crate) struct Graphics {
  pub(crate) device: wgpu::Device,
  pub(crate) queue: wgpu::Queue,
  /// The first fault of the device, once it has one.
  ///
  /// wgpu reports a fault to the device, whichever window drew what was at fault. A window stops
  /// drawing when it finds one here, and a device with one is handed to no further window.
  pub(crate) fault: Arc<OnceLock<String>>,
  /// The turn of the one window that builds under error scopes.
  ///
  /// wgpu keeps the scopes of a device as one stack, whichever thread pushes them.
  building: Mutex<()>,
  /// The turn of the one thread that polls the device.
  polling: Mutex<()>,
}

/// The device the windows of this plugin draw with.
///
/// It is here for as long as anything holds it.
static OPEN: Mutex<Weak<Graphics>> = Mutex::new(Weak::new());

impl Graphics {
  /// Returns the device the windows of this plugin draw with.
  ///
  /// One is made if nothing holds one, or if the one held has a fault or is found lost.
  ///
  /// # Errors
  ///
  /// Returns an error if no graphics adapter is found or the adapter opens no device.
  pub(crate) fn shared() -> Result<Arc<Self>, HandlerError> {
    // the lock is kept while a device is made, so that windows opening at once get the same
    // one. a panic under it leaves what it guards whole
    let mut open = OPEN.lock().unwrap_or_else(PoisonError::into_inner);
    if let Some(sound) = open.upgrade().filter(|held| held.sound()) {
      return Ok(sound);
    }
    let made = Arc::new(Self::new()?);
    *open = Arc::downgrade(&made);
    Ok(made)
  }

  /// Creates a device of its own.
  ///
  /// # Errors
  ///
  /// Returns an error if no graphics adapter is found or the adapter opens no device.
  pub(crate) fn new() -> Result<Self, HandlerError> {
    let adapter = adapter()?;
    let info = adapter.get_info();
    if info.device_type == wgpu::DeviceType::Cpu {
      warn!(
        "the editor draws without a graphics card: the adapter is {}",
        info.name
      );
    }
    // no more is asked of the adapter than every one has, apart from textures as large as
    // this one has, for a window's own size
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
      required_limits: wgpu::Limits::downlevel_defaults().using_resolution(adapter.limits()),
      ..Default::default()
    }))?;
    // wgpu would panic over a fault that nothing listens for
    let fault = Arc::new(OnceLock::new());
    device.on_uncaptured_error(Arc::new({
      let fault = Arc::clone(&fault);
      move |error: wgpu::Error| {
        // only the first is kept: the rest follow from it
        let _ = fault.set(error.to_string());
      }
    }));
    device.set_device_lost_callback({
      let fault = Arc::clone(&fault);
      move |reason, message| {
        // the same, and a device that is let go of is lost to nobody
        let _ = fault.set(format!(
          "the graphics device was lost: {reason:?} {message}"
        ));
      }
    });
    Ok(Self {
      device,
      queue,
      fault,
      building: Mutex::new(()),
      polling: Mutex::new(()),
    })
  }

  /// Polls the device, unless another thread is polling it.
  ///
  /// It is `None` if another thread is. That thread's poll reports every mapping that is ready,
  /// this thread's too.
  ///
  /// # Panics
  ///
  /// Panics if the device is lost, as wgpu's `Device::poll` does.
  pub(crate) fn poll(
    &self,
    poll: wgpu::PollType,
  ) -> Option<Result<wgpu::PollStatus, wgpu::PollError>> {
    // wgpu-core through 30.0.2 panics in `Device::maintain` if a wait times out and another
    // thread's poll or submit empties the queue before it looks. one thread polls at a time,
    // which stops the polls and leaves a submit stalled for a whole wait able to do it
    // https://github.com/gfx-rs/wgpu/pull/9958
    // TODO: poll without the turn once wgpu is on a release with that fix
    let turn = match self.polling.try_lock() {
      Ok(turn) => turn,
      // a poll that panicked left nothing behind the lock to be wrong
      Err(TryLockError::Poisoned(poisoned)) => poisoned.into_inner(),
      Err(TryLockError::WouldBlock) => return None,
    };
    let polled = self.device.poll(poll);
    drop(turn);
    Some(polled)
  }

  /// Looks for a fault and returns whether the device has none.
  ///
  /// A device lost while no window draws with it has told nobody. wgpu finds the loss when it
  /// next asks the device anything, which a poll does.
  fn sound(&self) -> bool {
    // wgpu panics over a loss it finds in a poll, once it has reported it. with another
    // thread polling, that one finds it
    let polled = catch_unwind(AssertUnwindSafe(|| self.poll(wgpu::PollType::Poll)));
    if polled.is_err() {
      // the loss itself is the fault if it was reported, and the first is the one kept
      let _ = (self.fault).set("the graphics device could not be polled".to_owned());
    }
    self.fault.get().is_none()
  }

  /// Returns what `build` makes with the device.
  ///
  /// wgpu reports a shader or pipeline it cannot build, and memory it cannot get, to whoever
  /// is listening for each kind, and panics if nobody is. While `build` runs, this listens for
  /// the whole device: an error of another window drawing on another thread at that time is
  /// taken for one of `build`.
  ///
  /// # Errors
  ///
  /// Returns an error if the device reports one while `build` runs.
  pub(crate) fn built<T>(&self, build: impl FnOnce() -> T) -> Result<T, wgpu::Error> {
    let kinds = [
      wgpu::ErrorFilter::Validation,
      wgpu::ErrorFilter::OutOfMemory,
      wgpu::ErrorFilter::Internal,
    ];
    // a panic in `build` leaves nothing behind the lock to be wrong
    let turn = (self.building.lock()).unwrap_or_else(PoisonError::into_inner);
    for kind in kinds {
      self.device.push_error_scope(kind);
    }
    // the scopes come off again whatever `build` does, or they would take every later fault
    let built = catch_unwind(AssertUnwindSafe(build));
    let mut wrong = None;
    for _ in kinds {
      wrong = pollster::block_on(self.device.pop_error_scope()).or(wrong);
    }
    drop(turn);
    let built = built.unwrap_or_else(|panic| resume_unwind(panic));
    wrong.map_or(Ok(built), Err)
  }
}

/// The instance whose adapters are tried first, made at the first asking.
///
/// It is kept for as long as the process lives, and every device is opened from it or from
/// [`LAST`].
// the nvidia driver on linux, seen with its release 595, no longer hands a process its
// graphics card once the process has let go of 17 vulkan instances. an adapter asked of a
// later one is the cpu's
// TODO: make an instance for each device again if a driver stops doing this
static FIRST: LazyLock<wgpu::Instance> =
  LazyLock::new(|| instance_of(wgpu::Backends::VULKAN | wgpu::Backends::METAL));
/// The instance whose adapters are tried when [`FIRST`] has none.
static LAST: LazyLock<wgpu::Instance> = LazyLock::new(|| instance_of(wgpu::Backends::DX12));

fn instance_of(backends: wgpu::Backends) -> wgpu::Instance {
  wgpu::Instance::new(&wgpu::InstanceDescriptor {
    backends,
    ..Default::default()
  })
}

/// Returns the graphics adapter a device is opened on.
fn adapter() -> Result<wgpu::Adapter, HandlerError> {
  // direct3d 12 is tried last. wine runs it on vulkan anyway, and its shader compiler builds
  // no shader with a sampler as naga writes it for direct3d, so there the editor needs vulkan
  adapter_of(&FIRST).or_else(|first| {
    adapter_of(&LAST).map_err(|second| {
      HandlerError::from_boxed(format!("{first}, and with direct3d 12: {second}").into())
    })
  })
}

/// Returns a graphics adapter of `instance`.
fn adapter_of(instance: &wgpu::Instance) -> Result<wgpu::Adapter, wgpu::RequestAdapterError> {
  pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
}

#[cfg(test)]
mod tests {
  use std::sync::Barrier;
  use std::time::Duration;

  use super::*;

  fn broken(device: &wgpu::Device) -> wgpu::ShaderModule {
    device.create_shader_module(wgpu::ShaderModuleDescriptor {
      label: None,
      source: wgpu::ShaderSource::Wgsl("fn broken(".into()),
    })
  }

  #[test]
  #[ignore = "needs a graphics device"]
  fn one_device_is_shared_for_as_long_as_it_is_held_and_has_no_fault() {
    let first = Graphics::shared().expect("a device opens");
    let second = Graphics::shared().expect("the device is shared");
    assert!(Arc::ptr_eq(&first, &second));
    drop(first);
    let third = Graphics::shared().expect("the device is shared");
    assert!(Arc::ptr_eq(&second, &third));
    // let go of by every holder it is gone
    let gone = Arc::downgrade(&third);
    drop((second, third));
    assert_eq!(gone.strong_count(), 0);

    let fresh = Graphics::shared().expect("a device opens");
    assert_eq!(Arc::strong_count(&fresh), 1);
    // one with a fault is handed out no more, and letting go of it leaves the next one shared
    fresh.fault.set("lost".to_owned()).expect("it had no fault");
    let next = Graphics::shared().expect("a device opens");
    assert!(!Arc::ptr_eq(&fresh, &next));
    drop(fresh);
    let same = Graphics::shared().expect("the device is shared");
    assert!(Arc::ptr_eq(&next, &same));

    // one that is gone while nothing draws with it, as a lost one is, is found out at the next
    // asking, which gets a device that draws
    same.device.destroy();
    assert_eq!(same.fault.get(), None);
    let after = Graphics::shared().expect("a device opens");
    assert!(!Arc::ptr_eq(&same, &after));
    assert_eq!(after.built(|| 7).ok(), Some(7));
    assert_eq!(after.fault.get(), None);
  }

  #[test]
  #[ignore = "needs a graphics device"]
  fn every_asking_for_an_adapter_gets_the_same_one() {
    // more askings than the nvidia driver on linux lets go of instances with a graphics card
    let name = || adapter().expect("an adapter is found").get_info().name;
    let first = name();
    for asking in 1..24 {
      assert_eq!(name(), first, "asking {asking}");
    }
  }

  #[test]
  #[ignore = "needs a graphics device"]
  fn what_goes_wrong_while_building_is_returned_and_is_no_fault_of_the_device() {
    let graphics = Graphics::new().expect("a device opens");
    assert_eq!(graphics.built(|| 7).ok(), Some(7));
    assert!(graphics.built(|| broken(&graphics.device)).is_err());
    // a build that panics leaves no scope behind to take the faults that follow it
    let panicked = catch_unwind(AssertUnwindSafe(|| graphics.built(|| panic!("no skin"))));
    assert!(panicked.is_err());
    assert_eq!(graphics.fault.get(), None);
    broken(&graphics.device);
    assert!(graphics.fault.get().is_some());
  }

  #[test]
  #[ignore = "needs a graphics device"]
  fn two_builds_at_once_each_get_their_own_errors() {
    let graphics = Graphics::new().expect("a device opens");
    for _ in 0..40 {
      let start = Barrier::new(2);
      // each is long enough to be under way when the other begins
      let build = |breaks: bool| {
        start.wait();
        graphics.built(|| {
          std::thread::sleep(Duration::from_millis(1));
          if breaks {
            broken(&graphics.device);
          }
          std::thread::sleep(Duration::from_millis(1));
        })
      };
      let (whole, failed) = std::thread::scope(|scope| {
        let whole = scope.spawn(|| build(false));
        let failed = scope.spawn(|| build(true));
        (whole.join(), failed.join())
      });
      assert!(whole.expect("the build ends").is_ok());
      assert!(failed.expect("the build ends").is_err());
    }
    assert_eq!(graphics.fault.get(), None);
  }
}
