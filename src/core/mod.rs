use crate::{eventloop::EventLoop, window_builder::WindowBuilder};

mod app_error;
mod builders;
mod context;
mod eventloop;
mod handle_loop;
mod handle_user;
mod messages;
mod utils;
mod webview;
mod webview_builder;
mod window;
mod window_builder;
/* 
#[derive(Debug, Clone)]
enum AppEvent {}

fn main() -> crate::app_error::Result<()> {

    
    let event_loop = EventLoop::<AppEvent>::new()?;
    let event_loop_handle = event_loop.handle();
    let window_builder = WindowBuilder::new();
    let _window_from_handler = event_loop_handle.create_window(window_builder)?;
    let window_builder = WindowBuilder::new();
    let _window_from_its_self = window_builder.build(&event_loop_handle)?;


    std::thread::spawn(move || {
        let window_builder = WindowBuilder::new();
        let _window = window_builder.build(&event_loop_handle.clone()).unwrap();
    });
    Ok(())
}
 */