#![cfg(target_os = "macos")]
use tracing::{trace, warn};

// Add PathBuf
use std::path::PathBuf;

use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2::{MainThreadMarker};
use objc2::{define_class, msg_send, MainThreadOnly};
use objc2_app_kit::{
    NSApplication, NSApplicationDelegate, NSApplicationDelegateReply,
};
use objc2_foundation::NSObjectProtocol;
use objc2_foundation::{NSArray, NSObject, NSURL};

define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "AppDelegate"]
    struct AppDelegate;

    unsafe impl NSObjectProtocol for AppDelegate {}

    unsafe impl NSApplicationDelegate for AppDelegate {
        #[unsafe(method(application:openFiles:))]
        #[allow(non_snake_case)]
        fn application_openFiles(&self, application: &NSApplication, files: &NSArray<NSURL>) {
            trace!("Triggered `application:openFiles:`");
            // Obtain MainThreadMarker. If AppDelegate stores mtm, use self.mtm.
            // Otherwise, if AppState is already initialized and holds one, it could be fetched,
            // or created anew if appropriate for NSURL::path.
            // For NSURL::path, it's often fine to create one if you're sure you're on the main thread.
            // let mtm =
            //     MainThreadMarker::new().expect("must be on main thread for application:openFiles:");

            let mut paths: Vec<PathBuf> = Vec::new();
            unsafe {
                for ns_url in files {
                    if let Some(ns_path_str) = ns_url.path() {
                        let path_str = ns_path_str.to_string();
                        let path_buf = PathBuf::from(path_str);
                        if path_buf.exists() {
                            paths.push(path_buf);
                        } else {
                            warn!(
                                "Received non-existent path from application:openFiles: {:?}",
                                path_buf
                            );
                        }
                    } else {
                        warn!(
                            "Received non-file URL or malformed path from application:openFiles:"
                        );
                    }
                }

                if paths.is_empty() {
                    // According to Apple's documentation, you should always call replyToOpenOrPrint.
                    // NSApplicationDelegateReply::Cancel indicates failure or no action.
                    application.replyToOpenOrPrint(NSApplicationDelegateReply::Cancel);
                } else {
                    application.replyToOpenOrPrint(NSApplicationDelegateReply::Success);

                    let _cloned_paths = paths;
                }
            }
            trace!("Completed `application:openFiles:`");
        }
    }
);

impl AppDelegate {
    fn new(mtm: MainThreadMarker) -> Retained<Self> {
        unsafe { msg_send![super(Self::alloc(mtm).set_ivars(())), init] }
    }
}

pub fn setup_app_delegates() {
    let mtm =
        MainThreadMarker::new().expect("on macOS, `EventLoop` must be created on the main thread!");

    let delegate = AppDelegate::new(mtm);
    let app = NSApplication::sharedApplication(mtm);
    app.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
}
