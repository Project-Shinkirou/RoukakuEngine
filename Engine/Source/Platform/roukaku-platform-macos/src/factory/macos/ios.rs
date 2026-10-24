//! UIKit owns the main thread for the application's lifetime. Unlike macOS,
//! winit's iOS event loop cannot be pumped and does not return from run_app.
use super::backend_setup::MacosRunnerApp;
use super::*;

pub struct IosHostRunner {
    event_loop: EventLoop<()>,
    app: MacosRunnerApp,
}

pub(super) fn document_bookmarks() -> Result<std::path::PathBuf, PlatformError> {
    let home = std::env::var_os("HOME")
        .ok_or_else(|| host_error("ios.documents", "iOS application sandbox HOME is missing"))?;
    Ok(std::path::PathBuf::from(home)
        .join("Library/Application Support/RoukakuPlayer/document-bookmarks"))
}

pub fn main_thread_host() -> Result<(IosHostRunner, crate::MacosPlatformFactory), PlatformError> {
    roukaku_platform::ios::restore_document_access(&document_bookmarks()?)?;
    roukaku_platform::ios::activate_audio()?;
    let pending = Arc::new(std::sync::Mutex::new(None));
    let event_loop = EventLoop::builder().build().map_err(|_| {
        host_error(
            "host.runner.create",
            "UIKit event loop could not be created on the main thread",
        )
    })?;
    event_loop.set_control_flow(ControlFlow::Wait);
    let wake = event_loop.create_proxy();
    Ok((
        IosHostRunner {
            event_loop,
            app: MacosRunnerApp {
                pending: pending.clone(),
                wake: wake.clone(),
                host: None,
            },
        },
        crate::MacosPlatformFactory {
            roots: None,
            pending,
            start_wake: Some(wake),
        },
    ))
}

impl IosHostRunner {
    pub fn run<M, F>(mut self, make_future: M) -> Result<(), PlatformError>
    where
        M: FnOnce() -> F + Send + 'static,
        F: Future<Output = Result<(), PlatformError>> + 'static,
    {
        let proxy = self.event_loop.create_proxy();
        // The session future does not need Send: it is constructed on its owner.
        // UIKit windows and Metal surfaces remain exclusively on the main thread.
        thread::Builder::new()
            .name("roukaku-ios-player".into())
            .spawn(move || {
                let result = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .map_err(|_| {
                        host_error("host.runner.run", "iOS player runtime could not start")
                    })
                    .and_then(|runtime| runtime.block_on(make_future()));
                if let Err(error) = result {
                    tracing::error!(event = "platform.ios.player.failed", error = %error);
                }
                // Session shutdown drains resources before completing this future.
                let _ = proxy.send_event(());
            })
            .map_err(|_| host_error("host.runner.run", "iOS player worker could not start"))?;
        self.event_loop
            .run_app(&mut self.app)
            .map_err(|_| host_error("host.runner.run", "UIKit event loop failed"))
    }
}
