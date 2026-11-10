use std::sync::OnceLock;
use std::sync::mpsc;
use std::time::Duration;

use block2::{DynBlock, RcBlock};
use futures::channel::mpsc::UnboundedSender;
use objc2::rc::Retained;
use objc2::runtime::{Bool, NSObject, NSObjectProtocol, ProtocolObject};
use objc2::{AnyThread, DefinedClass, define_class, msg_send};
use objc2_foundation::{NSBundle, NSError, NSString};
use objc2_user_notifications::{
    UNAuthorizationOptions, UNMutableNotificationContent, UNNotificationDefaultActionIdentifier, UNNotificationRequest,
    UNNotificationResponse, UNUserNotificationCenter, UNUserNotificationCenterDelegate,
};

use super::Refusal;

static DELEGATE: OnceLock<Retained<Delegate>> = OnceLock::new();

define_class!(
    #[unsafe(super(NSObject))]
    #[name = "CornercaseGpuiNotificationDelegate"]
    #[ivars = UnboundedSender<String>]
    struct Delegate;

    unsafe impl NSObjectProtocol for Delegate {}

    unsafe impl UNUserNotificationCenterDelegate for Delegate {
        #[unsafe(method(userNotificationCenter:didReceiveNotificationResponse:withCompletionHandler:))]
        fn opened(&self, _center: &UNUserNotificationCenter, response: &UNNotificationResponse, done: &DynBlock<dyn Fn()>) {
            if response.actionIdentifier().isEqualToString(unsafe { UNNotificationDefaultActionIdentifier }) {
                let _ = self.ivars().unbounded_send(response.notification().request().identifier().to_string());
            }
            done.call(());
        }
    }
);

pub(super) fn bundle_id() -> Option<String> {
    NSBundle::mainBundle().bundleIdentifier().map(|id| id.to_string())
}

pub(super) fn install(clicks: &UnboundedSender<String>) {
    if DELEGATE.get().is_some() {
        return;
    }
    let delegate: Retained<Delegate> = unsafe { msg_send![super(Delegate::alloc().set_ivars(clicks.clone())), init] };
    UNUserNotificationCenter::currentNotificationCenter().setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
    let _ = DELEGATE.set(delegate);
}

fn text(error: *mut NSError) -> Option<String> {
    unsafe { error.as_ref() }.map(|error| error.localizedDescription().to_string())
}

fn request(id: &str, title: &str, body: &str) -> Retained<UNNotificationRequest> {
    let content = UNMutableNotificationContent::new();
    content.setTitle(&NSString::from_str(title));
    content.setBody(&NSString::from_str(body));
    UNNotificationRequest::requestWithIdentifier_content_trigger(&NSString::from_str(id), &content, None)
}

fn add(center: &UNUserNotificationCenter, request: &UNNotificationRequest, tx: mpsc::Sender<Result<(), Refusal>>) {
    let added = RcBlock::new(move |error: *mut NSError| {
        let outcome = match text(error) {
            None => Ok(()),
            error => Err(Refusal { allowed: true, error }),
        };
        let _ = tx.send(outcome);
    });
    center.addNotificationRequest_withCompletionHandler(request, Some(&added));
}

pub(super) fn post((id, title, body): (&str, &str, &str), answer: Duration) -> Result<(), Refusal> {
    let center = UNUserNotificationCenter::currentNotificationCenter();
    let request = request(id, title, body);
    let (tx, rx) = mpsc::channel();
    let asked = RcBlock::new({
        let center = center.clone();
        move |granted: Bool, error: *mut NSError| {
            if granted.as_bool() {
                add(&center, &request, tx.clone());
            } else {
                let _ = tx.send(Err(Refusal { allowed: false, error: text(error) }));
            }
        }
    });
    center.requestAuthorizationWithOptions_completionHandler(UNAuthorizationOptions::Alert, &asked);
    rx.recv_timeout(answer).unwrap_or(Ok(()))
}
