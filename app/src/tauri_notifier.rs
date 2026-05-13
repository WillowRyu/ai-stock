use application::ports::notifier::{NotifyError, Notifier};
use async_trait::async_trait;
use tauri::AppHandle;
use tauri_plugin_notification::NotificationExt;

pub struct TauriNotifier {
    app: AppHandle,
}

impl TauriNotifier {
    pub fn new(app: AppHandle) -> Self { Self { app } }
}

#[async_trait]
impl Notifier for TauriNotifier {
    async fn notify(&self, title: &str, body: &str) -> Result<(), NotifyError> {
        self.app.notification()
            .builder()
            .title(title)
            .body(body)
            .show()
            .map_err(|e| NotifyError::Backend(e.to_string()))
    }
}
