//! Migration 16: create the `conversation_events` table.
//!
//! Stores individual events from agent PTY output streams so that late
//! subscribers can replay recent conversation history.  Each row captures
//! one discrete event (output chunk, tool use, thinking block, etc.) with
//! optional free-text content and a JSON metadata blob.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(ConversationEvents::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(ConversationEvents::Id).string().not_null().primary_key())
                    .col(ColumnDef::new(ConversationEvents::AgentId).string().not_null())
                    .col(ColumnDef::new(ConversationEvents::EventType).string().not_null())
                    .col(
                        ColumnDef::new(ConversationEvents::SessionNumber)
                            .big_integer()
                            .not_null()
                            .default(0),
                    )
                    .col(ColumnDef::new(ConversationEvents::Content).string().null())
                    .col(ColumnDef::new(ConversationEvents::Metadata).string().null())
                    .col(ColumnDef::new(ConversationEvents::CreatedAt).string().not_null())
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_conv_events_agent_created")
                    .table(ConversationEvents::Table)
                    .col(ConversationEvents::AgentId)
                    .col(ConversationEvents::CreatedAt)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_conv_events_agent_type")
                    .table(ConversationEvents::Table)
                    .col(ConversationEvents::AgentId)
                    .col(ConversationEvents::EventType)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Drop indexes first (SQLite requires this before dropping the table).
        manager.drop_index(Index::drop().name("idx_conv_events_agent_created").to_owned()).await?;
        manager.drop_index(Index::drop().name("idx_conv_events_agent_type").to_owned()).await?;
        manager.drop_table(Table::drop().table(ConversationEvents::Table).to_owned()).await
    }
}

#[derive(DeriveIden)]
enum ConversationEvents {
    Table,
    Id,
    AgentId,
    EventType,
    SessionNumber,
    Content,
    Metadata,
    CreatedAt,
}
