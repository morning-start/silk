/// 生成标准 `list()` 函数：`require_db()?` → `repo::find_all()` → `.map(response::from)`。
///
/// # 示例
///
/// ```ignore
/// impl_crud_list_get!(GatewayKeyResponse, GatewayKeyRepo, "网关 Key");
/// ```
#[macro_export]
macro_rules! impl_crud_list {
    ($response:ty, $repo:path, $find_label:expr) => {
        pub async fn list() -> Result<Vec<$response>, $crate::error::ServiceError> {
            let pool = $crate::error::require_db()?;
            let items = <$repo>::find_all(pool).await?;
            Ok(items.into_iter().map(<$response>::from).collect())
        }
    };
}

/// 生成标准 `get(id)` 函数：`require_db()?` → `require_found(repo::find_by_id())` → `response::from`。
#[macro_export]
macro_rules! impl_crud_get {
    ($response:ty, $repo:path, $find_label:expr) => {
        pub async fn get(id: String) -> Result<$response, $crate::error::ServiceError> {
            let pool = $crate::error::require_db()?;
            let item = $crate::error::require_found(
                <$repo>::find_by_id(pool, &id).await?,
                $find_label,
            )?;
            Ok(<$response>::from(item))
        }
    };
}

/// 生成标准 `delete(id)` 函数：`require_db()?` → `repo::delete()`。
#[macro_export]
macro_rules! impl_crud_delete {
    ($repo:path) => {
        pub async fn delete(id: String) -> Result<bool, $crate::error::ServiceError> {
            let pool = $crate::error::require_db()?;
            <$repo>::delete(pool, &id)
                .await
                .map_err($crate::error::ServiceError::from)
        }
    };
}
