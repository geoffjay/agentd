//! Storage layer for the knowledge service.
#![allow(dead_code)]
//!
//! [`KnowledgeStorage`] coordinates filesystem document files with SQLite
//! metadata via SeaORM. All path access goes through [`crate::fs::safe_doc_path`].
//!
//! ## Consistency model
//!
//! - **Create**: write file atomically → insert DB row (rollback = delete file).
//! - **Update**: write file atomically → update DB row (rollback = restore old content).
//! - **Delete**: delete DB row → delete file (orphaned file is harmless).
//! - **Conflict detection**: unique index `idx_documents_project_rel_path` bubbles
//!   up as `KnowledgeError::Conflict`.

use anyhow::Result;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, Condition, DatabaseConnection, EntityTrait,
    ModelTrait, PaginatorTrait, QueryFilter, QueryOrder, QuerySelect,
};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use crate::{
    entity::document,
    error::KnowledgeError,
    fs::{atomic_write, safe_doc_path},
    types::{
        CreateDocumentRequest, DoctorReport, Document, DocumentContent, PaginatedResponse,
        UpdateDocumentRequest,
    },
};

/// Wraps `Arc<KnowledgeStorage>` so handlers can share a single instance cheaply.
pub type SharedStorage = Arc<KnowledgeStorage>;

/// Persistent storage backend for the knowledge service.
pub struct KnowledgeStorage {
    pub(crate) db: DatabaseConnection,
    /// Root directory for document files.
    pub(crate) root: PathBuf,
}

impl KnowledgeStorage {
    /// Platform-specific database file path.
    pub fn get_db_path() -> Result<PathBuf> {
        agentd_common::storage::get_db_path("agentd-knowledge", "knowledge.db")
    }

    /// Creates a new storage instance with the default database path.
    pub async fn new(root: &Path) -> Result<Self> {
        let db_path = Self::get_db_path()?;
        Self::with_path(&db_path, root).await
    }

    /// Creates a new storage instance connected to `db_path`.
    pub async fn with_path(db_path: &Path, root: &Path) -> Result<Self> {
        use crate::migration::Migrator;
        use sea_orm_migration::prelude::MigratorTrait;
        let db = agentd_common::storage::create_connection(db_path).await?;
        Migrator::up(&db, None).await?;
        std::fs::create_dir_all(root)?;
        Ok(Self { db, root: root.to_path_buf() })
    }

    // -----------------------------------------------------------------------
    // Create
    // -----------------------------------------------------------------------

    /// Create a new document for `project_id`.
    ///
    /// # Errors
    ///
    /// - [`KnowledgeError::InvalidPath`] if `req.rel_path` fails safety checks.
    /// - [`KnowledgeError::Conflict`] if a document at that path already exists.
    /// - [`KnowledgeError::Other`] for I/O or DB errors.
    pub async fn create_document(
        &self,
        project_id: &str,
        req: CreateDocumentRequest,
        organization_id: Option<String>,
    ) -> std::result::Result<Document, KnowledgeError> {
        let abs_path = safe_doc_path(&self.root, project_id, &req.rel_path)?;

        // Derive title: explicit override or stem of the filename.
        let title = req.title.unwrap_or_else(|| {
            abs_path
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| req.rel_path.clone())
        });

        let content_bytes = req.content.as_bytes();
        let size_bytes = content_bytes.len() as i64;
        let now = chrono::Utc::now().to_rfc3339();
        let id = uuid::Uuid::new_v4().to_string();

        // 1. Write file atomically.
        atomic_write(&abs_path, content_bytes)
            .map_err(|e| KnowledgeError::Other(anyhow::anyhow!("write failed: {e}")))?;

        // 2. Insert DB row.
        let active = document::ActiveModel {
            id: Set(id.clone()),
            project_id: Set(project_id.to_string()),
            rel_path: Set(req.rel_path.clone()),
            title: Set(title.clone()),
            size_bytes: Set(size_bytes),
            created_at: Set(now.clone()),
            updated_at: Set(now.clone()),
            organization_id: Set(organization_id.clone()),
        };

        let result = document::Entity::insert(active).exec(&self.db).await;

        match result {
            Ok(_) => Ok(Document {
                id,
                project_id: project_id.to_string(),
                rel_path: req.rel_path,
                title,
                size_bytes,
                created_at: now.clone(),
                updated_at: now,
                organization_id,
            }),
            Err(e) => {
                // Attempt cleanup — best effort.
                let _ = std::fs::remove_file(&abs_path);
                Err(map_db_error(e, &req.rel_path))
            }
        }
    }

    // -----------------------------------------------------------------------
    // Read
    // -----------------------------------------------------------------------

    /// Fetch document metadata by ID.
    ///
    /// When `org` is `Some`, the lookup is additionally scoped to that
    /// organization (gateway traffic); when `None`, only `project_id` is
    /// enforced (trusted/local access).
    pub async fn get_document(
        &self,
        project_id: &str,
        doc_id: &str,
        org: Option<&str>,
    ) -> std::result::Result<Document, KnowledgeError> {
        let model = self.find_by_id(project_id, doc_id, org).await?;
        Ok(model_to_document(model))
    }

    /// Fetch document metadata + markdown content by ID.
    pub async fn get_document_content(
        &self,
        project_id: &str,
        doc_id: &str,
        org: Option<&str>,
    ) -> std::result::Result<DocumentContent, KnowledgeError> {
        let model = self.find_by_id(project_id, doc_id, org).await?;
        let abs_path = safe_doc_path(&self.root, project_id, &model.rel_path)?;
        let content = std::fs::read_to_string(&abs_path)
            .map_err(|e| KnowledgeError::Other(anyhow::anyhow!("read failed: {e}")))?;
        Ok(DocumentContent { document: model_to_document(model), content })
    }

    /// List documents for `project_id` with pagination.
    ///
    /// When `org` is `Some`, results are additionally scoped to that
    /// organization; when `None`, only `project_id` is enforced.
    pub async fn list_documents(
        &self,
        project_id: &str,
        org: Option<&str>,
        limit: u64,
        offset: u64,
    ) -> std::result::Result<PaginatedResponse<Document>, KnowledgeError> {
        let mut base_query =
            document::Entity::find().filter(document::Column::ProjectId.eq(project_id));
        if let Some(cond) = tenant_read_condition(org) {
            base_query = base_query.filter(cond);
        }
        let base_query = base_query.order_by_asc(document::Column::RelPath);

        let total = base_query
            .clone()
            .count(&self.db)
            .await
            .map_err(|e| KnowledgeError::Other(anyhow::anyhow!("count failed: {e}")))?;

        let models = base_query
            .limit(Some(limit))
            .offset(Some(offset))
            .all(&self.db)
            .await
            .map_err(|e| KnowledgeError::Other(anyhow::anyhow!("list failed: {e}")))?;

        Ok(PaginatedResponse {
            items: models.into_iter().map(model_to_document).collect(),
            total,
            limit,
            offset,
        })
    }

    // -----------------------------------------------------------------------
    // Update
    // -----------------------------------------------------------------------

    /// Update an existing document.
    ///
    /// Supports optimistic concurrency via `req.expected_updated_at`.
    pub async fn update_document(
        &self,
        project_id: &str,
        doc_id: &str,
        org: Option<&str>,
        req: UpdateDocumentRequest,
    ) -> std::result::Result<Document, KnowledgeError> {
        let model = self.find_by_id(project_id, doc_id, org).await?;

        // Optimistic concurrency check.
        if let Some(ref expected) = req.expected_updated_at {
            if &model.updated_at != expected {
                return Err(KnowledgeError::Conflict(format!(
                    "document {doc_id} was modified (expected updated_at={expected}, \
                     actual={})",
                    model.updated_at
                )));
            }
        }

        let abs_path = safe_doc_path(&self.root, project_id, &model.rel_path)?;

        // Capture old content for rollback.
        let old_content = std::fs::read(&abs_path)
            .map_err(|e| KnowledgeError::Other(anyhow::anyhow!("read for backup failed: {e}")))?;

        let new_content = req.content.as_deref().unwrap_or("");
        let new_size = new_content.len() as i64;
        let now = chrono::Utc::now().to_rfc3339();
        let new_title = req.title.clone().unwrap_or_else(|| model.title.clone());

        // Write new content.
        if req.content.is_some() {
            atomic_write(&abs_path, new_content.as_bytes())
                .map_err(|e| KnowledgeError::Other(anyhow::anyhow!("write failed: {e}")))?;
        }

        // Update DB row.
        let mut active: document::ActiveModel = model.clone().into();
        active.title = Set(new_title.clone());
        if req.content.is_some() {
            active.size_bytes = Set(new_size);
        }
        active.updated_at = Set(now.clone());

        let result = active.update(&self.db).await;

        match result {
            Ok(updated) => Ok(model_to_document(updated)),
            Err(e) => {
                // Rollback filesystem write.
                if req.content.is_some() {
                    let _ = atomic_write(&abs_path, &old_content);
                }
                Err(KnowledgeError::Other(anyhow::anyhow!("update failed: {e}")))
            }
        }
    }

    // -----------------------------------------------------------------------
    // Delete
    // -----------------------------------------------------------------------

    /// Delete a document by ID (removes both DB row and file).
    pub async fn delete_document(
        &self,
        project_id: &str,
        doc_id: &str,
        org: Option<&str>,
    ) -> std::result::Result<(), KnowledgeError> {
        let model = self.find_by_id(project_id, doc_id, org).await?;
        let abs_path = safe_doc_path(&self.root, project_id, &model.rel_path)?;

        // Delete DB row first — orphaned file is harmless.
        model
            .delete(&self.db)
            .await
            .map_err(|e| KnowledgeError::Other(anyhow::anyhow!("delete failed: {e}")))?;

        // Best-effort file removal.
        let _ = std::fs::remove_file(&abs_path);
        Ok(())
    }

    /// Delete ALL documents for `project_id` (called on project deletion).
    ///
    /// When `org` is `Some`, only that organization's documents under the
    /// project are removed; when `None`, all documents for the project are.
    pub async fn bulk_delete_project(
        &self,
        project_id: &str,
        org: Option<&str>,
    ) -> std::result::Result<(), KnowledgeError> {
        // Load all documents so we can clean up files too.
        let mut find_query =
            document::Entity::find().filter(document::Column::ProjectId.eq(project_id));
        if let Some(o) = org {
            find_query = find_query.filter(document::Column::OrganizationId.eq(o));
        }
        let models = find_query
            .all(&self.db)
            .await
            .map_err(|e| KnowledgeError::Other(anyhow::anyhow!("list for bulk delete: {e}")))?;

        // Delete all DB rows.
        let mut delete_query =
            document::Entity::delete_many().filter(document::Column::ProjectId.eq(project_id));
        if let Some(o) = org {
            delete_query = delete_query.filter(document::Column::OrganizationId.eq(o));
        }
        delete_query
            .exec(&self.db)
            .await
            .map_err(|e| KnowledgeError::Other(anyhow::anyhow!("bulk delete failed: {e}")))?;

        // Best-effort cleanup of files.
        for model in &models {
            if let Ok(abs_path) = safe_doc_path(&self.root, project_id, &model.rel_path) {
                let _ = std::fs::remove_file(&abs_path);
            }
        }

        // Remove the project directory if empty.
        let project_dir = self.root.join(project_id);
        let _ = std::fs::remove_dir(&project_dir); // only removes if empty
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Doctor / reconciliation
    // -----------------------------------------------------------------------

    /// Reconcile the DB vs the filesystem for `project_id`.
    ///
    /// Returns a [`DoctorReport`] describing:
    /// - `missing_files` — DB rows whose markdown file is absent from disk.
    /// - `orphaned_files` — disk files under the project directory that have
    ///   no matching DB row.
    ///
    /// When `fix = true` the method also:
    /// - Deletes DB rows for missing files.
    /// - Deletes orphaned disk files.
    ///
    /// Emits the `knowledge_missing_files` gauge metric with the number
    /// of missing-file divergences detected (before any fixing).
    pub async fn doctor(
        &self,
        project_id: &str,
        fix: bool,
    ) -> std::result::Result<DoctorReport, KnowledgeError> {
        // 1. Collect all DB rows for the project.
        let models = document::Entity::find()
            .filter(document::Column::ProjectId.eq(project_id))
            .all(&self.db)
            .await
            .map_err(|e| KnowledgeError::Other(anyhow::anyhow!("doctor db list: {e}")))?;

        // 2. Build the set of rel_paths known to the DB.
        let db_paths: std::collections::HashMap<String, String> =
            models.iter().map(|m| (m.rel_path.clone(), m.id.clone())).collect();

        // 3. Scan disk for all .md files under <root>/<project_id>/.
        let project_dir = self.root.join(project_id);
        let disk_paths = collect_md_files(&project_dir);

        // 4. Find DB rows whose files are missing from disk.
        let mut missing_files: Vec<String> = Vec::new();
        for rel_path in db_paths.keys() {
            let abs = project_dir.join(rel_path.replace('/', std::path::MAIN_SEPARATOR_STR));
            if !abs.exists() {
                missing_files.push(rel_path.clone());
            }
        }

        // 5. Find disk files that have no DB row.
        let mut orphaned_files: Vec<String> = Vec::new();
        for disk_rel in &disk_paths {
            if !db_paths.contains_key(disk_rel) {
                orphaned_files.push(disk_rel.clone());
            }
        }

        // Emit metric. Gauge tracks the current snapshot count, so it uses a
        // plain noun (no Prometheus-reserved `_total` suffix, which is for
        // monotonic counters — see the distinct `knowledge_missing_file_total`
        // counter in api.rs that increments on each missing-file read).
        metrics::gauge!("knowledge_missing_files").set(missing_files.len() as f64);

        let mut fixed = 0u32;

        if fix {
            // Remove DB rows for missing files.
            for rel_path in &missing_files {
                let affected = document::Entity::delete_many()
                    .filter(document::Column::ProjectId.eq(project_id))
                    .filter(document::Column::RelPath.eq(rel_path.as_str()))
                    .exec(&self.db)
                    .await
                    .map(|r| r.rows_affected)
                    .unwrap_or(0);
                fixed += affected as u32;
            }

            // Remove orphaned disk files.
            for rel_path in &orphaned_files {
                let abs = project_dir.join(rel_path.replace('/', std::path::MAIN_SEPARATOR_STR));
                if std::fs::remove_file(&abs).is_ok() {
                    fixed += 1;
                }
            }
        }

        Ok(DoctorReport { missing_files, orphaned_files, fixed })
    }

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    async fn find_by_id(
        &self,
        project_id: &str,
        doc_id: &str,
        org: Option<&str>,
    ) -> std::result::Result<document::Model, KnowledgeError> {
        let mut query = document::Entity::find_by_id(doc_id.to_string())
            .filter(document::Column::ProjectId.eq(project_id));
        if let Some(cond) = tenant_read_condition(org) {
            query = query.filter(cond);
        }
        query
            .one(&self.db)
            .await
            .map_err(|e| KnowledgeError::Other(anyhow::anyhow!("db error: {e}")))?
            .ok_or_else(|| KnowledgeError::NotFound(format!("document {doc_id}")))
    }
}

// ---------------------------------------------------------------------------
// Conversions
// ---------------------------------------------------------------------------

fn model_to_document(m: document::Model) -> Document {
    Document {
        id: m.id,
        project_id: m.project_id,
        rel_path: m.rel_path,
        title: m.title,
        size_bytes: m.size_bytes,
        created_at: m.created_at,
        updated_at: m.updated_at,
        organization_id: m.organization_id,
    }
}

/// Recursively collect all `.md` file paths under `dir`, returning them as
/// relative path strings (using `/` separators, matching `rel_path` in the DB).
fn collect_md_files(dir: &Path) -> Vec<String> {
    let mut out = Vec::new();
    collect_md_files_inner(dir, dir, &mut out);
    out
}

fn collect_md_files_inner(root: &Path, current: &Path, out: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(current) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        // `is_dir()` follows symlinks; guarding on `!is_symlink()` prevents a
        // circular symlink under the user-configurable knowledge root from
        // recursing infinitely (stack overflow).
        if path.is_dir() && !path.is_symlink() {
            collect_md_files_inner(root, &path, out);
        } else if path.extension().is_some_and(|e| e == "md") {
            if let Ok(rel) = path.strip_prefix(root) {
                // Normalise to forward-slash separators.
                let rel_str = rel.to_string_lossy().replace(std::path::MAIN_SEPARATOR, "/");
                out.push(rel_str);
            }
        }
    }
}

/// Build the org-scoping [`Condition`] for read queries.
///
/// When `org` is `Some`, the condition is:
///
/// ```sql
/// WHERE organization_id = <org> OR organization_id IS NULL
/// ```
///
/// The `OR IS NULL` arm implements the NULL-row transition policy documented in
/// `agentd-common::tenant`: rows created before multi-tenancy have a `NULL`
/// `organization_id` and must remain readable by every authenticated tenant
/// until an operator runs the backfill command.
///
/// When `org` is `None` (trusted/local access), no org filter is added and the
/// caller sees all rows for the project.
fn tenant_read_condition(org: Option<&str>) -> Option<Condition> {
    org.map(|o| {
        Condition::any()
            .add(document::Column::OrganizationId.eq(o))
            .add(document::Column::OrganizationId.is_null())
    })
}

/// Map a SeaORM database error to a `KnowledgeError`.
///
/// Detects unique constraint violations and surfaces them as `Conflict`.
fn map_db_error(e: sea_orm::DbErr, rel_path: &str) -> KnowledgeError {
    let msg = e.to_string();
    if msg.contains("UNIQUE constraint failed") {
        KnowledgeError::Conflict(format!("document at '{rel_path}' already exists"))
    } else {
        KnowledgeError::Other(anyhow::anyhow!("{e}"))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    const PROJ: &str = "550e8400-e29b-41d4-a716-446655440001";

    async fn make_storage(tmp: &TempDir) -> KnowledgeStorage {
        let db_path = tmp.path().join("test.db");
        let root = tmp.path().join("docs");
        KnowledgeStorage::with_path(&db_path, &root).await.expect("storage init")
    }

    fn create_req(rel_path: &str, content: &str) -> CreateDocumentRequest {
        CreateDocumentRequest {
            rel_path: rel_path.to_string(),
            title: None,
            content: content.to_string(),
        }
    }

    #[tokio::test]
    async fn test_create_and_get() {
        let tmp = tempfile::tempdir().unwrap();
        let s = make_storage(&tmp).await;
        let doc = s.create_document(PROJ, create_req("hello.md", "# Hello"), None).await.unwrap();

        assert_eq!(doc.rel_path, "hello.md");
        assert_eq!(doc.title, "hello");
        assert_eq!(doc.size_bytes, 7);

        let fetched = s.get_document(PROJ, &doc.id, None).await.unwrap();
        assert_eq!(fetched.id, doc.id);
    }

    #[tokio::test]
    async fn test_get_content() {
        let tmp = tempfile::tempdir().unwrap();
        let s = make_storage(&tmp).await;
        let doc = s
            .create_document(PROJ, create_req("notes.md", "# Notes\ncontent here"), None)
            .await
            .unwrap();

        let with_content = s.get_document_content(PROJ, &doc.id, None).await.unwrap();
        assert_eq!(with_content.content, "# Notes\ncontent here");
    }

    #[tokio::test]
    async fn test_unique_constraint_conflict() {
        let tmp = tempfile::tempdir().unwrap();
        let s = make_storage(&tmp).await;
        s.create_document(PROJ, create_req("dup.md", "first"), None).await.unwrap();
        let err = s.create_document(PROJ, create_req("dup.md", "second"), None).await.unwrap_err();
        assert!(matches!(err, KnowledgeError::Conflict(_)), "expected Conflict, got {err:?}");
    }

    #[tokio::test]
    async fn test_not_found() {
        let tmp = tempfile::tempdir().unwrap();
        let s = make_storage(&tmp).await;
        let err =
            s.get_document(PROJ, "00000000-0000-0000-0000-000000000000", None).await.unwrap_err();
        assert!(matches!(err, KnowledgeError::NotFound(_)));
    }

    #[tokio::test]
    async fn test_list_documents() {
        let tmp = tempfile::tempdir().unwrap();
        let s = make_storage(&tmp).await;
        s.create_document(PROJ, create_req("a.md", "alpha"), None).await.unwrap();
        s.create_document(PROJ, create_req("b.md", "beta"), None).await.unwrap();
        s.create_document(PROJ, create_req("c.md", "gamma"), None).await.unwrap();

        let page = s.list_documents(PROJ, None, 2, 0).await.unwrap();
        assert_eq!(page.total, 3);
        assert_eq!(page.items.len(), 2);

        let page2 = s.list_documents(PROJ, None, 2, 2).await.unwrap();
        assert_eq!(page2.items.len(), 1);
    }

    #[tokio::test]
    async fn test_update_document() {
        let tmp = tempfile::tempdir().unwrap();
        let s = make_storage(&tmp).await;
        let doc = s.create_document(PROJ, create_req("update.md", "initial"), None).await.unwrap();

        let updated = s
            .update_document(
                PROJ,
                &doc.id,
                None,
                UpdateDocumentRequest {
                    content: Some("updated content".to_string()),
                    title: Some("Updated Title".to_string()),
                    expected_updated_at: None,
                },
            )
            .await
            .unwrap();

        assert_eq!(updated.title, "Updated Title");
        assert_eq!(updated.size_bytes, 15); // "updated content".len()

        let with_content = s.get_document_content(PROJ, &doc.id, None).await.unwrap();
        assert_eq!(with_content.content, "updated content");
    }

    #[tokio::test]
    async fn test_update_optimistic_concurrency() {
        let tmp = tempfile::tempdir().unwrap();
        let s = make_storage(&tmp).await;
        let doc = s.create_document(PROJ, create_req("oc.md", "v1"), None).await.unwrap();

        let err = s
            .update_document(
                PROJ,
                &doc.id,
                None,
                UpdateDocumentRequest {
                    content: Some("v2".to_string()),
                    title: None,
                    expected_updated_at: Some("1970-01-01T00:00:00+00:00".to_string()),
                },
            )
            .await
            .unwrap_err();

        assert!(matches!(err, KnowledgeError::Conflict(_)));
    }

    #[tokio::test]
    async fn test_delete_document() {
        let tmp = tempfile::tempdir().unwrap();
        let s = make_storage(&tmp).await;
        let doc = s.create_document(PROJ, create_req("del.md", "bye"), None).await.unwrap();

        s.delete_document(PROJ, &doc.id, None).await.unwrap();

        let err = s.get_document(PROJ, &doc.id, None).await.unwrap_err();
        assert!(matches!(err, KnowledgeError::NotFound(_)));
    }

    #[tokio::test]
    async fn test_bulk_delete_project() {
        let tmp = tempfile::tempdir().unwrap();
        let s = make_storage(&tmp).await;
        s.create_document(PROJ, create_req("x.md", "x"), None).await.unwrap();
        s.create_document(PROJ, create_req("y.md", "y"), None).await.unwrap();

        s.bulk_delete_project(PROJ, None).await.unwrap();

        let page = s.list_documents(PROJ, None, 100, 0).await.unwrap();
        assert_eq!(page.total, 0);
    }

    #[tokio::test]
    async fn test_org_scoping_isolates_documents() {
        let tmp = tempfile::tempdir().unwrap();
        let s = make_storage(&tmp).await;

        // Same project, two different organizations.
        let a = s
            .create_document(PROJ, create_req("a.md", "owned by org-a"), Some("org-a".to_string()))
            .await
            .unwrap();
        s.create_document(PROJ, create_req("b.md", "owned by org-b"), Some("org-b".to_string()))
            .await
            .unwrap();

        // org-a sees only its own document.
        let page_a = s.list_documents(PROJ, Some("org-a"), 100, 0).await.unwrap();
        assert_eq!(page_a.total, 1);
        assert_eq!(page_a.items[0].rel_path, "a.md");

        // org-b cannot fetch org-a's document by id.
        let err = s.get_document(PROJ, &a.id, Some("org-b")).await.unwrap_err();
        assert!(matches!(err, KnowledgeError::NotFound(_)));

        // Unscoped (trusted/local) access sees everything in the project.
        let page_all = s.list_documents(PROJ, None, 100, 0).await.unwrap();
        assert_eq!(page_all.total, 2);

        // Bulk delete scoped to org-a leaves org-b's document intact.
        s.bulk_delete_project(PROJ, Some("org-a")).await.unwrap();
        let remaining = s.list_documents(PROJ, None, 100, 0).await.unwrap();
        assert_eq!(remaining.total, 1);
        assert_eq!(remaining.items[0].rel_path, "b.md");
    }

    // -----------------------------------------------------------------------
    // KB-7: tenant scoping and NULL-row transition policy
    // -----------------------------------------------------------------------

    /// Docs with `organization_id = NULL` (pre-tenancy / legacy) must be
    /// visible to any tenant-scoped request during the transition window.
    #[tokio::test]
    async fn test_null_org_doc_visible_to_any_tenant() {
        let tmp = tempfile::tempdir().unwrap();
        let s = make_storage(&tmp).await;

        // Legacy doc: no org assigned (NULL).
        let legacy =
            s.create_document(PROJ, create_req("legacy.md", "old content"), None).await.unwrap();
        assert!(legacy.organization_id.is_none());

        // Any authenticated tenant can list it.
        let page_a = s.list_documents(PROJ, Some("org-a"), 100, 0).await.unwrap();
        assert_eq!(page_a.total, 1, "org-a should see the NULL-org legacy doc");
        assert_eq!(page_a.items[0].rel_path, "legacy.md");

        let page_b = s.list_documents(PROJ, Some("org-b"), 100, 0).await.unwrap();
        assert_eq!(page_b.total, 1, "org-b should also see the NULL-org legacy doc");
    }

    /// Any tenant can fetch a NULL-org doc by ID (transition period access).
    #[tokio::test]
    async fn test_null_org_doc_get_by_any_tenant() {
        let tmp = tempfile::tempdir().unwrap();
        let s = make_storage(&tmp).await;

        let legacy =
            s.create_document(PROJ, create_req("shared.md", "# Shared"), None).await.unwrap();

        // Both orgs can fetch it by ID.
        let fetched_a = s.get_document(PROJ, &legacy.id, Some("org-a")).await.unwrap();
        assert_eq!(fetched_a.id, legacy.id);

        let fetched_b = s.get_document(PROJ, &legacy.id, Some("org-b")).await.unwrap();
        assert_eq!(fetched_b.id, legacy.id);
    }

    /// Org-scoped docs are NOT visible to another org (strict cross-tenant isolation).
    #[tokio::test]
    async fn test_cross_tenant_isolation() {
        let tmp = tempfile::tempdir().unwrap();
        let s = make_storage(&tmp).await;

        let doc_a = s
            .create_document(PROJ, create_req("a.md", "a"), Some("org-a".to_string()))
            .await
            .unwrap();
        let doc_b = s
            .create_document(PROJ, create_req("b.md", "b"), Some("org-b".to_string()))
            .await
            .unwrap();

        // org-a list sees only its own doc (no NULL-org docs here).
        let page_a = s.list_documents(PROJ, Some("org-a"), 100, 0).await.unwrap();
        assert_eq!(page_a.total, 1);
        assert_eq!(page_a.items[0].rel_path, "a.md");

        // org-b list sees only its own doc.
        let page_b = s.list_documents(PROJ, Some("org-b"), 100, 0).await.unwrap();
        assert_eq!(page_b.total, 1);
        assert_eq!(page_b.items[0].rel_path, "b.md");

        // org-b cannot fetch org-a's doc by ID.
        let err = s.get_document(PROJ, &doc_a.id, Some("org-b")).await.unwrap_err();
        assert!(matches!(err, KnowledgeError::NotFound(_)), "org-b must not see org-a's doc");

        // org-a cannot fetch org-b's doc by ID.
        let err = s.get_document(PROJ, &doc_b.id, Some("org-a")).await.unwrap_err();
        assert!(matches!(err, KnowledgeError::NotFound(_)), "org-a must not see org-b's doc");
    }

    /// An org-scoped `list` returns both its own docs AND legacy NULL-org docs,
    /// but NOT docs owned by another org.
    #[tokio::test]
    async fn test_tenant_list_mixes_own_and_null_org_docs() {
        let tmp = tempfile::tempdir().unwrap();
        let s = make_storage(&tmp).await;

        // One legacy doc (NULL org) and one from each org.
        s.create_document(PROJ, create_req("legacy.md", "legacy"), None).await.unwrap();
        s.create_document(PROJ, create_req("a.md", "a"), Some("org-a".to_string())).await.unwrap();
        s.create_document(PROJ, create_req("b.md", "b"), Some("org-b".to_string())).await.unwrap();

        // org-a sees: legacy.md + a.md (not b.md).
        let page_a = s.list_documents(PROJ, Some("org-a"), 100, 0).await.unwrap();
        assert_eq!(page_a.total, 2);
        let paths_a: Vec<&str> = page_a.items.iter().map(|d| d.rel_path.as_str()).collect();
        assert!(paths_a.contains(&"legacy.md"), "org-a must see the legacy doc");
        assert!(paths_a.contains(&"a.md"), "org-a must see its own doc");
        assert!(!paths_a.contains(&"b.md"), "org-a must NOT see org-b's doc");

        // org-b sees: legacy.md + b.md (not a.md).
        let page_b = s.list_documents(PROJ, Some("org-b"), 100, 0).await.unwrap();
        assert_eq!(page_b.total, 2);
        let paths_b: Vec<&str> = page_b.items.iter().map(|d| d.rel_path.as_str()).collect();
        assert!(paths_b.contains(&"legacy.md"), "org-b must see the legacy doc");
        assert!(paths_b.contains(&"b.md"), "org-b must see its own doc");
        assert!(!paths_b.contains(&"a.md"), "org-b must NOT see org-a's doc");
    }

    /// Org-scoped `bulk_delete_project` (gc) removes only that org's docs;
    /// NULL-org (legacy) docs are left untouched.
    #[tokio::test]
    async fn test_bulk_delete_org_scoped_leaves_null_org_docs() {
        let tmp = tempfile::tempdir().unwrap();
        let s = make_storage(&tmp).await;

        let legacy =
            s.create_document(PROJ, create_req("legacy.md", "legacy"), None).await.unwrap();
        s.create_document(PROJ, create_req("a.md", "a"), Some("org-a".to_string())).await.unwrap();

        // GC scoped to org-a.
        s.bulk_delete_project(PROJ, Some("org-a")).await.unwrap();

        // The legacy (NULL-org) doc must survive.
        let remaining = s.list_documents(PROJ, None, 100, 0).await.unwrap();
        assert_eq!(remaining.total, 1);
        assert_eq!(remaining.items[0].id, legacy.id, "legacy doc must survive org-scoped gc");
    }

    // -----------------------------------------------------------------------
    // Doctor tests
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_doctor_clean_project_reports_no_divergences() {
        let tmp = tempfile::tempdir().unwrap();
        let s = make_storage(&tmp).await;
        s.create_document(PROJ, create_req("docs/a.md", "alpha"), None).await.unwrap();
        s.create_document(PROJ, create_req("docs/b.md", "beta"), None).await.unwrap();

        let report = s.doctor(PROJ, false).await.unwrap();
        assert!(report.missing_files.is_empty(), "expected no missing files");
        assert!(report.orphaned_files.is_empty(), "expected no orphaned files");
        assert_eq!(report.fixed, 0);
    }

    #[tokio::test]
    async fn test_doctor_detects_missing_file() {
        let tmp = tempfile::tempdir().unwrap();
        let s = make_storage(&tmp).await;
        let doc = s.create_document(PROJ, create_req("orphan.md", "x"), None).await.unwrap();

        // Delete the file on disk directly (simulates a lost file).
        let file_path = tmp.path().join("docs").join(PROJ).join("orphan.md");
        std::fs::remove_file(&file_path).unwrap();

        let report = s.doctor(PROJ, false).await.unwrap();
        assert_eq!(report.missing_files, vec![doc.rel_path]);
        assert!(report.orphaned_files.is_empty());
        assert_eq!(report.fixed, 0, "fix=false should not repair");
    }

    #[tokio::test]
    async fn test_doctor_detects_orphaned_file() {
        let tmp = tempfile::tempdir().unwrap();
        let s = make_storage(&tmp).await;

        // Write a file that has no DB row.
        let project_dir = tmp.path().join("docs").join(PROJ);
        std::fs::create_dir_all(&project_dir).unwrap();
        std::fs::write(project_dir.join("stray.md"), b"orphan").unwrap();

        let report = s.doctor(PROJ, false).await.unwrap();
        assert!(report.missing_files.is_empty());
        assert_eq!(report.orphaned_files, vec!["stray.md".to_string()]);
        assert_eq!(report.fixed, 0);
    }

    #[tokio::test]
    async fn test_doctor_fix_removes_missing_db_row() {
        let tmp = tempfile::tempdir().unwrap();
        let s = make_storage(&tmp).await;
        let doc = s.create_document(PROJ, create_req("gone.md", "bye"), None).await.unwrap();

        // Remove file on disk.
        let file_path = tmp.path().join("docs").join(PROJ).join("gone.md");
        std::fs::remove_file(&file_path).unwrap();

        let report = s.doctor(PROJ, true).await.unwrap();
        assert_eq!(report.missing_files.len(), 1);
        assert!(report.fixed >= 1, "expected at least one fix");

        // DB row should now be gone.
        let err = s.get_document(PROJ, &doc.id, None).await.unwrap_err();
        assert!(matches!(err, KnowledgeError::NotFound(_)));
    }

    #[tokio::test]
    async fn test_doctor_fix_removes_orphaned_file() {
        let tmp = tempfile::tempdir().unwrap();
        let s = make_storage(&tmp).await;

        // Write an orphaned file with no DB row.
        let project_dir = tmp.path().join("docs").join(PROJ);
        std::fs::create_dir_all(&project_dir).unwrap();
        let orphan = project_dir.join("ghost.md");
        std::fs::write(&orphan, b"ghost").unwrap();

        let report = s.doctor(PROJ, true).await.unwrap();
        assert_eq!(report.orphaned_files.len(), 1);
        assert!(report.fixed >= 1, "expected at least one fix");
        assert!(!orphan.exists(), "orphaned file should have been deleted");
    }

    // -----------------------------------------------------------------------
    // End-to-end lifecycle test
    // -----------------------------------------------------------------------

    /// Full round-trip: create → list → confirm on disk → update → delete.
    #[tokio::test]
    async fn test_full_lifecycle_e2e() {
        let tmp = tempfile::tempdir().unwrap();
        let s = make_storage(&tmp).await;
        let proj = "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee";

        // Create a document.
        let doc = s
            .create_document(proj, create_req("guide.md", "# Guide\n\nInitial content."), None)
            .await
            .unwrap();
        assert_eq!(doc.rel_path, "guide.md");
        assert_eq!(doc.title, "guide");

        // File must exist on disk.
        let disk_path = tmp.path().join("docs").join(proj).join("guide.md");
        assert!(disk_path.exists(), "file should exist after create");
        assert_eq!(std::fs::read_to_string(&disk_path).unwrap(), "# Guide\n\nInitial content.");

        // List returns the new document.
        let page = s.list_documents(proj, None, 100, 0).await.unwrap();
        assert_eq!(page.total, 1);
        assert_eq!(page.items[0].id, doc.id);

        // Get metadata.
        let fetched = s.get_document(proj, &doc.id, None).await.unwrap();
        assert_eq!(fetched.rel_path, "guide.md");

        // Get with content.
        let with_content = s.get_document_content(proj, &doc.id, None).await.unwrap();
        assert_eq!(with_content.content, "# Guide\n\nInitial content.");

        // Update content.
        let updated = s
            .update_document(
                proj,
                &doc.id,
                None,
                UpdateDocumentRequest {
                    content: Some("# Guide\n\nUpdated content.".to_string()),
                    title: Some("Guide (revised)".to_string()),
                    expected_updated_at: None,
                },
            )
            .await
            .unwrap();
        assert_eq!(updated.title, "Guide (revised)");
        assert_eq!(std::fs::read_to_string(&disk_path).unwrap(), "# Guide\n\nUpdated content.");

        // Doctor with no divergences.
        let report = s.doctor(proj, false).await.unwrap();
        assert!(report.missing_files.is_empty());
        assert!(report.orphaned_files.is_empty());

        // Delete.
        s.delete_document(proj, &doc.id, None).await.unwrap();
        assert!(!disk_path.exists(), "file should be gone after delete");
        let err = s.get_document(proj, &doc.id, None).await.unwrap_err();
        assert!(matches!(err, KnowledgeError::NotFound(_)));

        // List is now empty.
        let empty = s.list_documents(proj, None, 100, 0).await.unwrap();
        assert_eq!(empty.total, 0);
    }

    #[tokio::test]
    async fn test_atomic_write_no_partial_file() {
        // Ensure the file is written only if the full content is available.
        let tmp = tempfile::tempdir().unwrap();
        let target = tmp.path().join("atomic_test.md");
        atomic_write(&target, b"complete content").unwrap();
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "complete content");
        // No leftover temp files.
        let entries: Vec<_> = std::fs::read_dir(tmp.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().starts_with(".tmp-"))
            .collect();
        assert!(entries.is_empty(), "leftover temp files found");
    }
}
