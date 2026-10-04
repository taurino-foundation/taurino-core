use crate::schema::{PhysicalPosition, PhysicalRect, PhysicalSize};

use crate::MonitorExt;

impl MonitorExt for tao::monitor::MonitorHandle {
  fn work_area(&self) -> PhysicalRect<i32, u32> {
    use tao::platform::windows::MonitorHandleExtWindows;
    use windows::Win32::Graphics::Gdi::{GetMonitorInfoW, HMONITOR, MONITORINFO};
    let mut monitor_info = MONITORINFO {
      cbSize: std::mem::size_of::<MONITORINFO>() as u32,
      ..Default::default()
    };
    let status = unsafe { GetMonitorInfoW(HMONITOR(self.hmonitor() as _), &mut monitor_info) };
    if status.as_bool() {
      PhysicalRect {
        size: PhysicalSize::new(
          (monitor_info.rcWork.right - monitor_info.rcWork.left) as u32,
          (monitor_info.rcWork.bottom - monitor_info.rcWork.top) as u32,
        ),
        position: PhysicalPosition::new(monitor_info.rcWork.left, monitor_info.rcWork.top),
      }
    } else {
      PhysicalRect {
        size: self.size(),
        position: self.position(),
      }
    }
  }
}

#[cfg(desktop)]
pub fn calculate_window_center_position(
  window_size: tao::dpi::PhysicalSize<u32>,
  target_monitor: tao::monitor::MonitorHandle,
) -> tao::dpi::PhysicalPosition<i32> {
  let work_area = target_monitor.work_area();

  tao::dpi::PhysicalPosition::new(
    (work_area.size.width as i32 - window_size.width as i32) / 2 + work_area.position.x,
    (work_area.size.height as i32 - window_size.height as i32) / 2 + work_area.position.y,
  )
}
