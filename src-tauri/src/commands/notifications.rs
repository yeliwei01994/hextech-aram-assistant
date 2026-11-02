use tauri::AppHandle;
use tauri_plugin_notification::{NotificationExt, PermissionState};

fn permission_label(state: PermissionState) -> &'static str {
    match state {
        PermissionState::Granted => "granted",
        PermissionState::Denied => "denied",
        PermissionState::Prompt | PermissionState::PromptWithRationale => "prompt",
    }
}

#[tauri::command]
pub fn get_notification_permission(app: AppHandle) -> Result<&'static str, String> {
    app.notification()
        .permission_state()
        .map(permission_label)
        .map_err(|error| format!("读取 Windows 通知权限失败：{error}"))
}

#[tauri::command]
pub fn request_notification_permission(app: AppHandle) -> Result<&'static str, String> {
    app.notification()
        .request_permission()
        .map(permission_label)
        .map_err(|error| format!("请求 Windows 通知权限失败：{error}"))
}

#[tauri::command]
pub fn send_app_notification(
    app: AppHandle,
    title: String,
    body: String,
) -> Result<bool, String> {
    if app.notification().permission_state().map_err(|error| error.to_string())?
        != PermissionState::Granted
    {
        return Ok(false);
    }

    app.notification()
        .builder()
        .title(title)
        .body(body)
        .show()
        .map(|()| true)
        .map_err(|error| format!("发送 Windows 通知失败：{error}"))
}
