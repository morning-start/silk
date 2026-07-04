use tauri::State;

use crate::application::routing_service::{
    self, CreateRoutingRulePayload, RoutingRuleResponse, UpdateRoutingRulePayload,
};
use crate::AppState;

#[tauri::command]
pub async fn list_routing_rules() -> Result<Vec<RoutingRuleResponse>, String> {
    routing_service::list().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_routing_rule(id: String) -> Result<RoutingRuleResponse, String> {
    routing_service::get(id).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn create_routing_rule(
    state: State<'_, AppState>,
    payload: CreateRoutingRulePayload,
) -> Result<RoutingRuleResponse, String> {
    routing_service::create(state.inner(), payload).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn update_routing_rule(
    state: State<'_, AppState>,
    id: String,
    payload: UpdateRoutingRulePayload,
) -> Result<RoutingRuleResponse, String> {
    routing_service::update(state.inner(), id, payload).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_routing_rule(state: State<'_, AppState>, id: String) -> Result<bool, String> {
    routing_service::delete(state.inner(), id).await.map_err(|e| e.to_string())
}
