use tauri::State;

use crate::application::group_service::{
    self, AddMemberPayload, CreateGroupPayload, GroupWithMembersResponse, UpdateGroupPayload,
    UpdateMemberPayload,
};
use crate::models::{GroupMember, ProviderGroup};
use crate::AppState;

#[tauri::command]
pub async fn list_groups(_state: State<'_, AppState>) -> Result<Vec<ProviderGroup>, String> {
    group_service::list(_state).await
}

#[tauri::command]
pub async fn find_groups_by_model(
    state: State<'_, AppState>,
    model_name: String,
) -> Result<Vec<ProviderGroup>, String> {
    group_service::find_by_model(state, model_name).await
}

#[tauri::command]
pub async fn get_group(
    state: State<'_, AppState>,
    id: String,
) -> Result<GroupWithMembersResponse, String> {
    group_service::get(state, id).await
}

#[tauri::command]
pub async fn create_group(
    state: State<'_, AppState>,
    payload: CreateGroupPayload,
) -> Result<ProviderGroup, String> {
    group_service::create(state, payload).await
}

#[tauri::command]
pub async fn update_group(
    state: State<'_, AppState>,
    id: String,
    payload: UpdateGroupPayload,
) -> Result<ProviderGroup, String> {
    group_service::update(state, id, payload).await
}

#[tauri::command]
pub async fn delete_group(state: State<'_, AppState>, id: String) -> Result<bool, String> {
    group_service::delete(state, id).await
}

#[tauri::command]
pub async fn add_group_member(
    state: State<'_, AppState>,
    group_id: String,
    payload: AddMemberPayload,
) -> Result<GroupMember, String> {
    group_service::add_member(state, group_id, payload).await
}

#[tauri::command]
pub async fn update_group_member(
    state: State<'_, AppState>,
    id: String,
    payload: UpdateMemberPayload,
) -> Result<GroupMember, String> {
    group_service::update_member(state, id, payload).await
}

#[tauri::command]
pub async fn remove_group_member(state: State<'_, AppState>, id: String) -> Result<bool, String> {
    group_service::remove_member(state, id).await
}
