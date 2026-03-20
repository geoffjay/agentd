//! Initial migration: create `rooms`, `participants`, and `messages` tables.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // ----------------------------------------------------------------
        // rooms
        // ----------------------------------------------------------------
        manager
            .create_table(
                Table::create()
                    .table(Rooms::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Rooms::Id).string().not_null().primary_key())
                    .col(ColumnDef::new(Rooms::Name).string().not_null().unique_key())
                    .col(ColumnDef::new(Rooms::Topic).string().null())
                    .col(ColumnDef::new(Rooms::Description).string().null())
                    .col(ColumnDef::new(Rooms::RoomType).string().not_null())
                    .col(ColumnDef::new(Rooms::CreatedBy).string().not_null())
                    .col(ColumnDef::new(Rooms::CreatedAt).string().not_null())
                    .col(ColumnDef::new(Rooms::UpdatedAt).string().not_null())
                    .to_owned(),
            )
            .await?;

        // ----------------------------------------------------------------
        // participants
        // ----------------------------------------------------------------
        manager
            .create_table(
                Table::create()
                    .table(Participants::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Participants::Id).string().not_null().primary_key())
                    .col(ColumnDef::new(Participants::RoomId).string().not_null())
                    .col(ColumnDef::new(Participants::Identifier).string().not_null())
                    .col(ColumnDef::new(Participants::Kind).string().not_null())
                    .col(ColumnDef::new(Participants::DisplayName).string().not_null())
                    .col(ColumnDef::new(Participants::Role).string().not_null())
                    .col(ColumnDef::new(Participants::JoinedAt).string().not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_participants_room_id")
                            .from(Participants::Table, Participants::RoomId)
                            .to(Rooms::Table, Rooms::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // Unique constraint: a participant can only join a room once.
        manager
            .create_index(
                Index::create()
                    .name("idx_participants_room_identifier")
                    .table(Participants::Table)
                    .col(Participants::RoomId)
                    .col(Participants::Identifier)
                    .unique()
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        // ----------------------------------------------------------------
        // messages
        // ----------------------------------------------------------------
        manager
            .create_table(
                Table::create()
                    .table(Messages::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Messages::Id).string().not_null().primary_key())
                    .col(ColumnDef::new(Messages::RoomId).string().not_null())
                    .col(ColumnDef::new(Messages::SenderId).string().not_null())
                    .col(ColumnDef::new(Messages::SenderName).string().not_null())
                    .col(ColumnDef::new(Messages::SenderKind).string().not_null())
                    .col(ColumnDef::new(Messages::Content).string().not_null())
                    .col(ColumnDef::new(Messages::Metadata).string().not_null())
                    .col(ColumnDef::new(Messages::ReplyTo).string().null())
                    .col(ColumnDef::new(Messages::Status).string().not_null())
                    .col(ColumnDef::new(Messages::CreatedAt).string().not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_messages_room_id")
                            .from(Messages::Table, Messages::RoomId)
                            .to(Rooms::Table, Rooms::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // Composite index on (room_id, created_at) for efficient room history queries.
        manager
            .create_index(
                Index::create()
                    .name("idx_messages_room_created_at")
                    .table(Messages::Table)
                    .col(Messages::RoomId)
                    .col(Messages::CreatedAt)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Drop in reverse FK-dependency order.
        manager.drop_table(Table::drop().table(Messages::Table).to_owned()).await?;
        manager.drop_table(Table::drop().table(Participants::Table).to_owned()).await?;
        manager.drop_table(Table::drop().table(Rooms::Table).to_owned()).await?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Iden enums
// ---------------------------------------------------------------------------

#[derive(DeriveIden)]
enum Rooms {
    Table,
    Id,
    Name,
    Topic,
    Description,
    RoomType,
    CreatedBy,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum Participants {
    Table,
    Id,
    RoomId,
    Identifier,
    Kind,
    DisplayName,
    Role,
    JoinedAt,
}

#[derive(DeriveIden)]
enum Messages {
    Table,
    Id,
    RoomId,
    SenderId,
    SenderName,
    SenderKind,
    Content,
    Metadata,
    ReplyTo,
    Status,
    CreatedAt,
}
