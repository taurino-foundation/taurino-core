use crate::schema::{LogicalPosition, LogicalSize, PhysicalRect};

use gtk::gdk::prelude::MonitorExt;
use tao::platform::unix::MonitorHandleExtUnix;

impl crate::platforms::MonitorExt for tao::monitor::MonitorHandle {
  fn work_area(&self) -> PhysicalRect<i32, u32> {
    let rect = self.gdk_monitor().workarea();
    let scale_factor = self.scale_factor();
    PhysicalRect {
      size: LogicalSize::new(rect.width() as u32, rect.height() as u32).to_physical(scale_factor),
      position: LogicalPosition::new(rect.x(), rect.y()).to_physical(scale_factor),
    }
  }
}
