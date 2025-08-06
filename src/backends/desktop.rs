use winit::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
    platform::run_return::EventLoopExtRunReturn,
};

use crate::api::Window;

pub struct DesktopWindow {
    event_loop: Option<EventLoop<()>>,
    window: Option<winit::window::Window>,
    running: bool,
}

impl Window for DesktopWindow {
    fn new(title: &str, width: u32, height: u32) -> Self {
        let event_loop = EventLoop::new();
        let window = WindowBuilder::new()
            .with_title(title)
            .with_inner_size(winit::dpi::LogicalSize::new(width, height))
            .build(&event_loop)
            .unwrap();

        Self {
            event_loop: Some(event_loop),
            window: Some(window),
            running: true,
        }
    }

    fn update(&mut self) {
        if let Some(mut event_loop) = self.event_loop.take() {
            let running = &mut self.running;
            let _window = self.window.as_ref().unwrap();

            event_loop.run_return(|event, _, control_flow| {
                match event {
                    Event::WindowEvent { event, .. } => match event {
                        WindowEvent::CloseRequested => {
                            *control_flow = ControlFlow::Exit;
                            *running = false;
                        }
                        _ => {}
                    },
                    _ => {}
                }
            });

            self.event_loop = Some(event_loop);
        }
    }

    fn is_open(&self) -> bool {
        self.running
    }
}