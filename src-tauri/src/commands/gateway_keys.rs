use tauri::State;

use crate::application::gateway_key_service as gks;
use crate::application::gateway_key_service::GatewayKeyResponse;
use crate::AppState;

#[tauri::command]
pub async fn get_builtin_gateway_key(
    _state: State<'_, AppState>,
) -> Result<GatewayKeyResponse, String> {
    gks::get_builtin_key().map_err(|e| e.to_string())
}

/// 刷新内置 Key，重新生成并写入文件
#[tauri::command]
pub async fn reset_builtin_gateway_key(
    _state: State<'_, AppState>,
) -> Result<GatewayKeyResponse, String> {
    gks::reset_builtin_key().map_err(|e| e.to_string())
}