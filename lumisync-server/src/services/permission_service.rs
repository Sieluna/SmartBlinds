use std::sync::Arc;

use lumisync_api::RegionRole;
use sqlx::Error;

use crate::configs::Storage;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceType {
    Region,
    Group,
    Device,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Permission(u32);

impl Permission {
    // Common permissions (bits 0-7)
    pub const VIEW: Permission = Permission(1 << 0);
    pub const UPDATE: Permission = Permission(1 << 1);
    pub const DELETE: Permission = Permission(1 << 2);

    // Region specific permissions (bits 8-15)
    pub const REGION_MANAGE_DEVICES: Permission = Permission(1 << 8);
    pub const REGION_ASSIGN_PERMISSIONS: Permission = Permission(1 << 9);

    // Group specific permissions (bits 16-23)
    pub const GROUP_INVITE_MEMBERS: Permission = Permission(1 << 16);
    pub const GROUP_REMOVE_MEMBERS: Permission = Permission(1 << 17);
    pub const GROUP_MANAGE_SETTINGS: Permission = Permission(1 << 18);

    // Device specific permissions (bits 24-31)
    pub const DEVICE_CONTROL: Permission = Permission(1 << 24);
    pub const DEVICE_UPDATE_SETTINGS: Permission = Permission(1 << 25);

    // Common permission combinations
    pub const DEVICE_FULL: Permission = Permission(
        Self::VIEW.0 | Self::DEVICE_CONTROL.0 | Self::DEVICE_UPDATE_SETTINGS.0 | Self::DELETE.0,
    );
    pub const REGION_FULL: Permission = Permission(
        Self::VIEW.0
            | Self::UPDATE.0
            | Self::DELETE.0
            | Self::REGION_MANAGE_DEVICES.0
            | Self::REGION_ASSIGN_PERMISSIONS.0,
    );
    pub const GROUP_FULL: Permission = Permission(
        Self::VIEW.0
            | Self::UPDATE.0
            | Self::DELETE.0
            | Self::GROUP_INVITE_MEMBERS.0
            | Self::GROUP_REMOVE_MEMBERS.0
            | Self::GROUP_MANAGE_SETTINGS.0,
    );

    pub fn has(&self, permission: Permission) -> bool {
        (self.0 & permission.0) == permission.0
    }

    pub fn union(&self, other: Permission) -> Permission {
        Permission(self.0 | other.0)
    }
}

#[derive(Clone)]
pub struct PermissionService {
    storage: Arc<Storage>,
}

impl PermissionService {
    pub fn new(storage: Arc<Storage>) -> Self {
        Self { storage }
    }

    pub async fn is_admin(&self, user_id: i32) -> Result<bool, Error> {
        let result: Option<bool> =
            sqlx::query_scalar("SELECT role = 'admin' FROM users WHERE id = $1")
                .bind(user_id)
                .fetch_optional(self.storage.get_pool())
                .await?;

        Ok(result.unwrap_or(false))
    }

    pub async fn is_in_group(&self, user_id: i32, group_id: i32) -> Result<bool, Error> {
        let result: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM users_groups_link WHERE user_id = $1 AND group_id = $2 AND is_active = TRUE)"
        )
        .bind(user_id)
        .bind(group_id)
        .fetch_one(self.storage.get_pool())
        .await?;

        Ok(result)
    }

    pub async fn get_user_region_role(
        &self,
        user_id: i32,
        region_id: i32,
    ) -> Result<Option<RegionRole>, Error> {
        let user_region_role: Option<String> = sqlx::query_scalar(
            "SELECT role FROM users_regions_link WHERE user_id = $1 AND region_id = $2 AND is_active = TRUE"
        )
        .bind(user_id)
        .bind(region_id)
        .fetch_optional(self.storage.get_pool())
        .await?;

        if let Some(role) = user_region_role {
            return Ok(Some(role.into()));
        }

        if self.is_admin(user_id).await? {
            return Ok(Some(RegionRole::Owner));
        }

        let result: Option<bool> = sqlx::query_scalar(
            r#"
            SELECT EXISTS (
                SELECT 1 FROM users_groups_link ugl
                JOIN regions r ON r.group_id = ugl.group_id
                WHERE ugl.user_id = $1 AND r.id = $2 AND ugl.is_active = TRUE
            )"#,
        )
        .bind(user_id)
        .bind(region_id)
        .fetch_one(self.storage.get_pool())
        .await?;

        if result.unwrap_or(false) {
            return Ok(Some(RegionRole::Visitor));
        }

        Ok(None)
    }

    async fn get_user_permission(
        &self,
        user_id: i32,
        resource_type: ResourceType,
        resource_id: i32,
    ) -> Result<Permission, Error> {
        if self.is_admin(user_id).await? {
            match resource_type {
                ResourceType::Region => Ok(Permission::REGION_FULL),
                ResourceType::Group => Ok(Permission::GROUP_FULL),
                ResourceType::Device => Ok(Permission::DEVICE_FULL),
            }
        } else {
            match resource_type {
                ResourceType::Region => self.get_user_region_permission(user_id, resource_id).await,
                ResourceType::Group => self.get_user_group_permission(user_id, resource_id).await,
                ResourceType::Device => self.get_user_device_permission(user_id, resource_id).await,
            }
        }
    }

    async fn get_user_region_permission(
        &self,
        user_id: i32,
        region_id: i32,
    ) -> Result<Permission, Error> {
        let role = match self.get_user_region_role(user_id, region_id).await? {
            Some(r) => r,
            None => return Ok(Permission(0)),
        };

        Ok(match role {
            RegionRole::Owner => Permission::REGION_FULL,
            RegionRole::Visitor => Permission::VIEW,
        })
    }

    async fn get_user_group_permission(
        &self,
        user_id: i32,
        group_id: i32,
    ) -> Result<Permission, Error> {
        let is_in_group = self.is_in_group(user_id, group_id).await?;

        if !is_in_group {
            return Ok(Permission(0));
        }

        // TODO: Current simple implementation: group members only have view permission
        // Can be extended to query user's role in the group and assign permissions based on role
        Ok(Permission::VIEW)
    }

    async fn get_user_device_permission(
        &self,
        user_id: i32,
        device_id: i32,
    ) -> Result<Permission, Error> {
        let device_region =
            sqlx::query_scalar::<_, i32>("SELECT region_id FROM devices WHERE id = $1")
                .bind(device_id)
                .fetch_optional(self.storage.get_pool())
                .await?;

        let region_id = match device_region {
            Some(id) => id,
            None => return Ok(Permission(0)),
        };

        let region_permission = self.get_user_region_permission(user_id, region_id).await?;

        if region_permission.has(Permission::REGION_MANAGE_DEVICES) {
            return Ok(Permission::DEVICE_FULL);
        }

        if region_permission.has(Permission::VIEW) {
            return Ok(Permission::VIEW.union(Permission::DEVICE_CONTROL));
        }

        Ok(Permission(0))
    }

    pub async fn check_permission(
        &self,
        user_id: i32,
        resource_type: ResourceType,
        resource_id: i32,
        required: Permission,
    ) -> Result<bool, Error> {
        let permission = self
            .get_user_permission(user_id, resource_type, resource_id)
            .await?;
        Ok(permission.has(required))
    }

    pub async fn is_region_public(&self, region_id: i32) -> Result<bool, Error> {
        let result: Option<bool> =
            sqlx::query_scalar("SELECT is_public FROM regions WHERE id = $1")
                .bind(region_id)
                .fetch_optional(self.storage.get_pool())
                .await?;

        Ok(result.unwrap_or(false))
    }

    pub async fn can_user_access_region(
        &self,
        user_id: i32,
        region_id: i32,
    ) -> Result<bool, Error> {
        // Check each access condition separately for easier debugging
        // 1. Check if user is an administrator
        let is_admin: bool = sqlx::query_scalar("SELECT role = 'admin' FROM users WHERE id = $1")
            .bind(user_id)
            .fetch_optional(self.storage.get_pool())
            .await?
            .unwrap_or(false);

        if is_admin {
            return Ok(true);
        }

        // 2. Check if region is public
        let is_public: bool = sqlx::query_scalar("SELECT is_public FROM regions WHERE id = $1")
            .bind(region_id)
            .fetch_optional(self.storage.get_pool())
            .await?
            .unwrap_or(false);

        if is_public {
            return Ok(true);
        }

        // 3. Check if user has direct access to region
        let has_direct_access: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM users_regions_link WHERE user_id = $1 AND region_id = $2 AND is_active = TRUE)"
        )
        .bind(user_id)
        .bind(region_id)
        .fetch_one(self.storage.get_pool())
        .await?;

        if has_direct_access {
            return Ok(true);
        }

        // 4. Check if user is in the region's group
        let is_in_regions_group: bool = sqlx::query_scalar(
            r#"
            SELECT EXISTS (
                SELECT 1 
                FROM users_groups_link ugl
                JOIN regions r ON r.group_id = ugl.group_id
                WHERE ugl.user_id = $1 AND r.id = $2 AND ugl.is_active = TRUE
            )
            "#,
        )
        .bind(user_id)
        .bind(region_id)
        .fetch_one(self.storage.get_pool())
        .await?;

        Ok(is_in_regions_group)
    }

    pub async fn change_region_public_status(
        &self,
        user_id: i32,
        region_id: i32,
        is_public: bool,
    ) -> Result<bool, Error> {
        let has_permission = self
            .check_permission(user_id, ResourceType::Region, region_id, Permission::UPDATE)
            .await?;
        if !has_permission {
            return Ok(false);
        }

        let mut tx = self.storage.get_pool().begin().await?;

        sqlx::query("UPDATE regions SET is_public = $1 WHERE id = $2")
            .bind(is_public)
            .bind(region_id)
            .execute(&mut *tx)
            .await?;

        tx.commit().await?;

        Ok(true)
    }

    pub async fn assign_user_region_role(
        &self,
        admin_user_id: i32,
        target_user_id: i32,
        region_id: i32,
        role: &str,
    ) -> Result<bool, Error> {
        let has_permission = self
            .check_permission(
                admin_user_id,
                ResourceType::Region,
                region_id,
                Permission::REGION_ASSIGN_PERMISSIONS,
            )
            .await?;

        if !has_permission {
            return Ok(false);
        }

        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM users_regions_link WHERE user_id = $1 AND region_id = $2)"
        )
        .bind(target_user_id)
        .bind(region_id)
        .fetch_one(self.storage.get_pool())
        .await?;

        let mut tx = self.storage.get_pool().begin().await?;

        if exists {
            sqlx::query(
                "UPDATE users_regions_link SET role = $1, is_active = TRUE WHERE user_id = $2 AND region_id = $3"
            )
            .bind(role)
            .bind(target_user_id)
            .bind(region_id)
            .execute(&mut *tx)
            .await?;
        } else {
            sqlx::query(
                "INSERT INTO users_regions_link (user_id, region_id, role, joined_at, is_active) VALUES ($1, $2, $3, $4, $5)"
            )
            .bind(target_user_id)
            .bind(region_id)
            .bind(role)
            .bind(time::OffsetDateTime::now_utc())
            .bind(true)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;

        Ok(true)
    }

    pub async fn add_user_to_group(
        &self,
        admin_user_id: i32,
        target_user_id: i32,
        group_id: i32,
    ) -> Result<bool, Error> {
        let has_permission = self
            .check_permission(
                admin_user_id,
                ResourceType::Group,
                group_id,
                Permission::GROUP_INVITE_MEMBERS,
            )
            .await?;

        if !has_permission {
            return Ok(false);
        }

        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM users_groups_link WHERE user_id = $1 AND group_id = $2)",
        )
        .bind(target_user_id)
        .bind(group_id)
        .fetch_one(self.storage.get_pool())
        .await?;

        let mut tx = self.storage.get_pool().begin().await?;

        if exists {
            sqlx::query(
                "UPDATE users_groups_link SET is_active = TRUE WHERE user_id = $1 AND group_id = $2"
            )
            .bind(target_user_id)
            .bind(group_id)
            .execute(&mut *tx)
            .await?;
        } else {
            sqlx::query(
                "INSERT INTO users_groups_link (user_id, group_id, joined_at, is_active) VALUES ($1, $2, $3, $4)"
            )
            .bind(target_user_id)
            .bind(group_id)
            .bind(time::OffsetDateTime::now_utc())
            .bind(true)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;

        Ok(true)
    }

    pub async fn remove_user_from_group(
        &self,
        admin_user_id: i32,
        target_user_id: i32,
        group_id: i32,
    ) -> Result<bool, Error> {
        let has_permission = self
            .check_permission(
                admin_user_id,
                ResourceType::Group,
                group_id,
                Permission::GROUP_REMOVE_MEMBERS,
            )
            .await?;

        if !has_permission {
            return Ok(false);
        }

        let mut tx = self.storage.get_pool().begin().await?;

        sqlx::query(
            r#"
            UPDATE users_groups_link SET is_active = FALSE
            WHERE user_id = $1 AND group_id = $2
            "#,
        )
        .bind(target_user_id)
        .bind(group_id)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        Ok(true)
    }

    pub async fn get_accessible_regions(&self, user_id: i32) -> Result<Vec<i32>, Error> {
        let regions: Vec<i32> = sqlx::query_scalar(
            r#"
            SELECT DISTINCT r.id FROM regions r
            WHERE EXISTS (SELECT 1 FROM users WHERE id = $1 AND role = 'admin')
                OR r.is_public = TRUE
                OR EXISTS (SELECT 1 FROM users_regions_link WHERE user_id = $1 AND region_id = r.id AND is_active = TRUE)
                OR EXISTS (SELECT 1 FROM users_groups_link ugl WHERE ugl.user_id = $1 AND ugl.group_id = r.group_id AND ugl.is_active = TRUE)
            "#,
        )
        .bind(user_id)
        .fetch_all(self.storage.get_pool())
        .await?;

        Ok(regions)
    }
}

#[cfg(test)]
mod tests {
    use lumisync_api::{DeviceType, UserRole};
    use serde_json::json;

    use crate::tests::*;

    use super::*;

    #[tokio::test]
    async fn test_is_admin() {
        let storage = setup_test_db().await;
        let admin_user =
            create_test_user(storage.clone(), "admin@test.com", "test", &UserRole::Admin).await;
        let normal_user =
            create_test_user(storage.clone(), "normal@test.com", "test", &UserRole::User).await;

        let permission_service = PermissionService::new(storage.clone());

        assert!(permission_service.is_admin(admin_user.id).await.unwrap());
        assert!(!permission_service.is_admin(normal_user.id).await.unwrap());
    }

    #[tokio::test]
    async fn test_region_permissions() {
        let storage = setup_test_db().await;
        let admin_user =
            create_test_user(storage.clone(), "admin@test.com", "test", &UserRole::Admin).await;
        let normal_user =
            create_test_user(storage.clone(), "normal@test.com", "test", &UserRole::User).await;
        let owner_user =
            create_test_user(storage.clone(), "owner@test.com", "test", &UserRole::User).await;
        let visitor_user =
            create_test_user(storage.clone(), "visitor@test.com", "test", &UserRole::User).await;

        let group = create_test_group(storage.clone(), "test_group").await;
        create_test_user_group(storage.clone(), normal_user.id, group.id, true).await;

        let region = create_test_region(
            storage.clone(),
            group.id,
            "test_region",
            500,
            22.5,
            45.0,
            false,
        )
        .await;

        create_test_user_region(storage.clone(), owner_user.id, region.id, "owner").await;
        create_test_user_region(storage.clone(), visitor_user.id, region.id, "visitor").await;

        let permission_service = PermissionService::new(storage.clone());

        let admin_role = permission_service
            .get_user_region_role(admin_user.id, region.id)
            .await
            .unwrap();
        let owner_role = permission_service
            .get_user_region_role(owner_user.id, region.id)
            .await
            .unwrap();
        let visitor_role = permission_service
            .get_user_region_role(visitor_user.id, region.id)
            .await
            .unwrap();
        let regular_role = permission_service
            .get_user_region_role(normal_user.id, region.id)
            .await
            .unwrap();

        assert_eq!(admin_role, Some(RegionRole::Owner));
        assert_eq!(owner_role, Some(RegionRole::Owner));
        assert_eq!(visitor_role, Some(RegionRole::Visitor));
        assert_eq!(regular_role, Some(RegionRole::Visitor));

        assert!(
            permission_service
                .check_permission(
                    admin_user.id,
                    ResourceType::Region,
                    region.id,
                    Permission::REGION_FULL
                )
                .await
                .unwrap()
        );

        assert!(
            permission_service
                .check_permission(
                    owner_user.id,
                    ResourceType::Region,
                    region.id,
                    Permission::REGION_MANAGE_DEVICES
                )
                .await
                .unwrap()
        );

        assert!(
            permission_service
                .check_permission(
                    visitor_user.id,
                    ResourceType::Region,
                    region.id,
                    Permission::VIEW
                )
                .await
                .unwrap()
        );
        assert!(
            !permission_service
                .check_permission(
                    visitor_user.id,
                    ResourceType::Region,
                    region.id,
                    Permission::REGION_MANAGE_DEVICES
                )
                .await
                .unwrap()
        );
    }

    #[tokio::test]
    async fn test_public_region_access() {
        let storage = setup_test_db().await;
        let user1 =
            create_test_user(storage.clone(), "user1@test.com", "test", &UserRole::User).await;
        let user2 =
            create_test_user(storage.clone(), "user2@test.com", "test", &UserRole::User).await;
        let group = create_test_group(storage.clone(), "test_group").await;
        let private_region = create_test_region(
            storage.clone(),
            group.id,
            "private_region",
            500,
            22.5,
            45.0,
            false,
        )
        .await;

        let public_region = create_test_region(
            storage.clone(),
            group.id,
            "public_region",
            500,
            22.5,
            45.0,
            true,
        )
        .await;

        create_test_user_group(storage.clone(), user1.id, group.id, true).await;

        let permission_service = PermissionService::new(storage.clone());

        assert!(
            permission_service
                .can_user_access_region(user1.id, private_region.id)
                .await
                .unwrap()
        );
        assert!(
            permission_service
                .can_user_access_region(user1.id, public_region.id)
                .await
                .unwrap()
        );

        assert!(
            !permission_service
                .can_user_access_region(user2.id, private_region.id)
                .await
                .unwrap()
        );
        assert!(
            permission_service
                .can_user_access_region(user2.id, public_region.id)
                .await
                .unwrap()
        );

        assert!(
            permission_service
                .check_permission(
                    user1.id,
                    ResourceType::Region,
                    private_region.id,
                    Permission::VIEW
                )
                .await
                .unwrap()
        );
        assert!(
            !permission_service
                .check_permission(
                    user2.id,
                    ResourceType::Region,
                    private_region.id,
                    Permission::VIEW
                )
                .await
                .unwrap()
        );
    }

    #[tokio::test]
    async fn test_device_permissions() {
        let storage = setup_test_db().await;
        let owner_user =
            create_test_user(storage.clone(), "owner@test.com", "test", &UserRole::User).await;
        let visitor_user =
            create_test_user(storage.clone(), "visitor@test.com", "test", &UserRole::User).await;
        let group = create_test_group(storage.clone(), "test_group").await;

        let region = create_test_region(
            storage.clone(),
            group.id,
            "test_region",
            500,
            22.5,
            45.0,
            false,
        )
        .await;

        create_test_user_region(storage.clone(), owner_user.id, region.id, "owner").await;
        create_test_user_region(storage.clone(), visitor_user.id, region.id, "visitor").await;

        let device = create_test_device(
            storage.clone(),
            region.id,
            "test_device",
            &DeviceType::Window,
            json!({"status": "off"}),
        )
        .await;

        let permission_service = PermissionService::new(storage.clone());

        assert!(
            permission_service
                .check_permission(
                    owner_user.id,
                    ResourceType::Device,
                    device.id,
                    Permission::DEVICE_FULL
                )
                .await
                .unwrap()
        );

        assert!(
            permission_service
                .check_permission(
                    visitor_user.id,
                    ResourceType::Device,
                    device.id,
                    Permission::VIEW
                )
                .await
                .unwrap()
        );
        assert!(
            permission_service
                .check_permission(
                    visitor_user.id,
                    ResourceType::Device,
                    device.id,
                    Permission::DEVICE_CONTROL
                )
                .await
                .unwrap()
        );
        assert!(
            !permission_service
                .check_permission(
                    visitor_user.id,
                    ResourceType::Device,
                    device.id,
                    Permission::DEVICE_UPDATE_SETTINGS
                )
                .await
                .unwrap()
        );
    }
}
