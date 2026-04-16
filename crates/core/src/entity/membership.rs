//! SeaORM entity for the `memberships` junction table.
//!
//! Memberships implement the many-to-many relationship between [`super::user`]
//! and [`super::organization`]. The composite unique constraint
//! `(user_id, organization_id)` is enforced by the migration index
//! `uq_membership_user_org`.

use sea_orm::entity::prelude::*;

/// SeaORM model for the `memberships` table.
///
/// `role` is one of `"owner"`, `"admin"`, or `"member"`.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "memberships")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub user_id: String,
    pub organization_id: String,
    /// Membership role — `"owner"`, `"admin"`, or `"member"`.
    pub role: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::user::Entity",
        from = "Column::UserId",
        to = "super::user::Column::Id"
    )]
    User,
    #[sea_orm(
        belongs_to = "super::organization::Entity",
        from = "Column::OrganizationId",
        to = "super::organization::Column::Id"
    )]
    Organization,
}

impl Related<super::user::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::User.def()
    }
}

impl Related<super::organization::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Organization.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
