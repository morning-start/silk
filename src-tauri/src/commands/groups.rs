use serde::{Deserialize, Serialize};
use tauri::State;

use crate::models::{
    GroupMember, NewGroupMember, NewProviderGroup, ProviderGroup, UpdateGroupMember,
    UpdateProviderGroup,
};
use crate::persistence::GroupRepo;
use crate::AppState;

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

/// 获取所有分组
#[tauri::command]
pub async fn list_groups(state: State<'_, AppState>) -> Result<Vec<ProviderGroup>, String> {
    let pool = crate::get_db_pool().ok_or("数据库未初始化")?;
    GroupRepo::find_all_groups(pool)
        .await
        .map_err(|e| format!("查询分组失败: {e}"))
}

/// 根据模型名查找分组
#[tauri::command]
pub async fn find_groups_by_model(
    state: State<'_, AppState>,
    model_name: String,
) -> Result<Vec<ProviderGroup>, String> {
    let pool = crate::get_db_pool().ok_or("数据库未初始化")?;
    GroupRepo::find_groups_by_model(pool, &model_name)
        .await
        .map_err(|e| format!("查询分组失败: {e}"))
}

/// 获取单个分组（含成员）
#[tauri::command]
pub async fn get_group(
    state: State<'_, AppState>,
    id: String,
) -> Result<GroupWithMembersResponse, String> {
    let pool = crate::get_db_pool().ok_or("数据库未初始化")?;

    let group = GroupRepo::find_group_by_id(pool, &id)
        .await
        .map_err(|e| format!("查询分组失败: {e}"))?
        .ok_or("分组不存在")?;

    let members = GroupRepo::find_members_by_group(pool, &id)
        .await
        .map_err(|e| format!("查询成员失败: {e}"))?;

    Ok(GroupWithMembersResponse {
        group: group.into(),
        members: members.into_iter().map(|m| m.into()).collect(),
    })
}

/// 创建分组
#[tauri::command]
pub async fn create_group(
    state: State<'_, AppState>,
    payload: CreateGroupPayload,
) -> Result<ProviderGroup, String> {
    let pool = crate::get_db_pool().ok_or("数据库未初始化")?;

    let new = NewProviderGroup {
        name: payload.name,
        model_name: payload.model_name,
        strategy: payload.strategy,
        enabled: payload.enabled,
    };

    let group = GroupRepo::create_group(pool, &new)
        .await
        .map_err(|e| format!("创建分组失败: {e}"))?;

    // 重新加载分组
    state
        .gateway
        .group_manager
        .reload_group(pool, &group.id)
        .await
        .ok();

    Ok(group)
}

/// 更新分组
#[tauri::command]
pub async fn update_group(
    state: State<'_, AppState>,
    id: String,
    payload: UpdateGroupPayload,
) -> Result<ProviderGroup, String> {
    let pool = crate::get_db_pool().ok_or("数据库未初始化")?;

    let update = UpdateProviderGroup {
        name: payload.name,
        model_name: payload.model_name,
        strategy: payload.strategy,
        enabled: payload.enabled,
    };

    let group = GroupRepo::update_group(pool, &id, &update)
        .await
        .map_err(|e| format!("更新分组失败: {e}"))?
        .ok_or("分组不存在")?;

    // 重新加载分组
    state
        .gateway
        .group_manager
        .reload_group(pool, &id)
        .await
        .ok();

    Ok(group)
}

/// 删除分组
#[tauri::command]
pub async fn delete_group(state: State<'_, AppState>, id: String) -> Result<bool, String> {
    let pool = crate::get_db_pool().ok_or("数据库未初始化")?;
    let deleted = GroupRepo::delete_group(pool, &id)
        .await
        .map_err(|e| format!("删除分组失败: {e}"))?;

    if deleted {
        state.gateway.group_manager.reload_all(pool).await.ok();
    }

    Ok(deleted)
}

/// 添加分组成员
#[tauri::command]
pub async fn add_group_member(
    state: State<'_, AppState>,
    group_id: String,
    payload: AddMemberPayload,
) -> Result<GroupMember, String> {
    let pool = crate::get_db_pool().ok_or("数据库未初始化")?;

    let new = NewGroupMember {
        group_id: group_id.clone(),
        provider_id: payload.provider_id,
        weight: payload.weight,
    };

    let member = GroupRepo::add_member(pool, &new)
        .await
        .map_err(|e| format!("添加成员失败: {e}"))?;

    // 重新加载分组
    state
        .gateway
        .group_manager
        .reload_group(pool, &member.group_id)
        .await
        .ok();

    Ok(member)
}

/// 更新分组成员
#[tauri::command]
pub async fn update_group_member(
    state: State<'_, AppState>,
    id: String,
    payload: UpdateMemberPayload,
) -> Result<GroupMember, String> {
    let pool = crate::get_db_pool().ok_or("数据库未初始化")?;

    let update = UpdateGroupMember {
        weight: payload.weight,
        enabled: payload.enabled,
    };

    let member = GroupRepo::update_member(pool, &id, &update)
        .await
        .map_err(|e| format!("更新成员失败: {e}"))?
        .ok_or("成员不存在")?;

    // 重新加载分组
    state
        .gateway
        .group_manager
        .reload_group(pool, &member.group_id)
        .await
        .ok();

    Ok(member)
}

/// 移除分组成员
#[tauri::command]
pub async fn remove_group_member(state: State<'_, AppState>, id: String) -> Result<bool, String> {
    let pool = crate::get_db_pool().ok_or("数据库未初始化")?;

    // 获取 group_id 用于重载
    let member = sqlx::query_as::<_, GroupMember>(
        r#"SELECT id, group_id, provider_id, weight, enabled, created_at FROM group_members WHERE id = ?1"#,
    )
    .bind(&id)
    .fetch_optional(pool)
    .await
    .map_err(|e| format!("查询成员失败: {e}"))?;

    let group_id = member.map(|m| m.group_id.clone());

    let deleted = GroupRepo::remove_member(pool, &id)
        .await
        .map_err(|e| format!("移除成员失败: {e}"))?;

    if deleted {
        if let Some(gid) = group_id {
            state.gateway.group_manager.reload_group(pool, &gid).await.ok();
        }
    }

    Ok(deleted)
}

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, Clone)]
pub struct GroupWithMembersResponse {
    pub group: GroupInfo,
    pub members: Vec<MemberInfo>,
}

#[derive(Debug, Serialize, Clone)]
pub struct GroupInfo {
    pub id: String,
    pub name: String,
    pub model_name: String,
    pub strategy: String,
    pub enabled: bool,
    pub created_at: String,
    pub updated_at: String,
}

impl From<ProviderGroup> for GroupInfo {
    fn from(g: ProviderGroup) -> Self {
        Self {
            id: g.id,
            name: g.name,
            model_name: g.model_name,
            strategy: g.strategy,
            enabled: g.enabled != 0,
            created_at: g.created_at.to_string(),
            updated_at: g.updated_at.to_string(),
        }
    }
}

#[derive(Debug, Serialize, Clone)]
pub struct MemberInfo {
    pub id: String,
    pub group_id: String,
    pub provider_id: String,
    pub weight: i64,
    pub enabled: bool,
}

impl From<GroupMember> for MemberInfo {
    fn from(m: GroupMember) -> Self {
        Self {
            id: m.id,
            group_id: m.group_id,
            provider_id: m.provider_id,
            weight: m.weight,
            enabled: m.enabled != 0,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct CreateGroupPayload {
    pub name: String,
    pub model_name: String,
    pub strategy: Option<String>,
    pub enabled: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateGroupPayload {
    pub name: Option<String>,
    pub model_name: Option<String>,
    pub strategy: Option<String>,
    pub enabled: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct AddMemberPayload {
    pub provider_id: String,
    pub weight: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateMemberPayload {
    pub weight: Option<i64>,
    pub enabled: Option<bool>,
}
