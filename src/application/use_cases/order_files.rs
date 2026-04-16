use std::sync::Arc;

use tokio::io::AsyncRead;

use crate::application::errors::AppError;
use crate::domain::order_file::{NewOrderFile, OrderFile};
use crate::domain::repositories::
{
    FileStorage, MaterialRepository, OrderFileRepository, OrderRepository,
    UserRepository,
};
use crate::infrastructure::storage::local_fs::LocalFileStorage;
use crate::infrastructure::storage::upload::validate_filename;

pub struct UploadOrderFileUseCase<O, F>
where
    O: OrderRepository,
    F: OrderFileRepository,
{
    orders: Arc<O>,
    files: Arc<F>,
    storage: Arc<LocalFileStorage>,
}

impl<O, F> UploadOrderFileUseCase<O, F>
where
    O: OrderRepository,
    F: OrderFileRepository,
{
    pub fn new(orders: Arc<O>, files: Arc<F>, storage: Arc<LocalFileStorage>) -> Self
    {
        Self { orders, files, storage }
    }

    /// Authorize (student owner or any order if caller is admin),
    /// enforce per-order and global quotas, then stream bytes from the
    /// provided reader into the storage. Returns the persisted
    /// OrderFile row on success.
    pub async fn execute<R>(
        &self,
        order_id: i64,
        owner_user_id: i64,
        raw_filename: &str,
        reader: &mut R,
    ) -> Result<OrderFile, AppError>
    where
        R: AsyncRead + Unpin + Send,
    {
        let order = self.orders.find_by_id(order_id)?
            .ok_or_else(|| AppError::NotFound(format!("order {order_id}")))?;
        if order.user_id != owner_user_id
        {
            return Err(AppError::NotAuthorized);
        }

        let (clean_name, kind) = validate_filename(raw_filename)?;

        let cfg = self.storage.config();
        let existing = self.files.count_by_order(order_id)?;
        if existing >= cfg.max_files_per_order
        {
            return Err(AppError::InvalidInput("too many files for this order".to_owned()));
        }

        // Pessimistic pre-check: if the storage is already full enough
        // that adding one more max-sized file would overflow, refuse
        // now. This is best-effort under concurrency; a second post-
        // upload check below confirms the exact result.
        let stats = self.files.storage_stats()?;
        let used = u64::try_from(stats.total_bytes).unwrap_or(u64::MAX);
        if used.saturating_add(cfg.max_upload_bytes) > cfg.max_total_storage_bytes
        {
            return Err(AppError::Database("storage full".to_owned()));
        }

        let stored = self.storage.store_upload(reader, kind).await?;

        // Post-check using the real written size. Still racy under
        // concurrent uploads; accepted for the fablab usage profile.
        let stored_size = stored.size_bytes;
        if used.saturating_add(stored_size) > cfg.max_total_storage_bytes
        {
            let _ = self.storage.delete(&stored.stored_filename).await;
            return Err(AppError::Database("storage full".to_owned()));
        }

        let size_i64 = i64::try_from(stored_size).unwrap_or(i64::MAX);
        let row = self.files.create(NewOrderFile
        {
            order_id,
            original_filename: clean_name,
            stored_filename: stored.stored_filename.clone(),
            size_bytes: size_i64,
            mime_type: kind.mime_type().to_owned(),
        });

        match row
        {
            Ok(f) => Ok(f),
            Err(e) =>
            {
                let _ = self.storage.delete(&stored.stored_filename).await;
                Err(e.into())
            }
        }
    }
}

/// Look up an order file and verify that the caller is allowed to read
/// it. Students may only read their own files. Returns NotFound (not
/// Forbidden) for non-owners to avoid leaking existence.
pub struct DownloadOrderFileUseCase<O, F>
where
    O: OrderRepository,
    F: OrderFileRepository,
{
    orders: Arc<O>,
    files: Arc<F>,
}

impl<O, F> DownloadOrderFileUseCase<O, F>
where
    O: OrderRepository,
    F: OrderFileRepository,
{
    pub fn new(orders: Arc<O>, files: Arc<F>) -> Self
    {
        Self { orders, files }
    }

    pub fn authorize(
        &self,
        file_id: i64,
        is_admin: bool,
        caller_user_id: Option<i64>,
    ) -> Result<OrderFile, AppError>
    {
        let file = match self.files.find_by_id(file_id)?
        {
            Some(f) => f,
            None => return Err(AppError::NotFound("file".to_owned())),
        };

        if is_admin
        {
            return Ok(file);
        }

        let order = match self.orders.find_by_id(file.order_id)?
        {
            Some(o) => o,
            None => return Err(AppError::NotFound("file".to_owned())),
        };
        match caller_user_id
        {
            Some(uid) if uid == order.user_id => Ok(file),
            _ => Err(AppError::NotFound("file".to_owned())),
        }
    }
}

/// Delete a file: admin only. Removes the DB row first, then the disk
/// file. Disk failures are logged but do not fail the call: the DB is
/// the source of truth and orphaned bytes can be reclaimed later.
pub struct DeleteOrderFileUseCase<F>
where
    F: OrderFileRepository,
{
    files: Arc<F>,
    storage: Arc<dyn FileStorage>,
}

impl<F> DeleteOrderFileUseCase<F>
where
    F: OrderFileRepository,
{
    pub fn new(files: Arc<F>, storage: Arc<dyn FileStorage>) -> Self
    {
        Self { files, storage }
    }

    pub async fn execute(&self, file_id: i64, is_admin: bool) -> Result<(), AppError>
    {
        if !is_admin
        {
            return Err(AppError::NotAuthorized);
        }

        let file = self.files.find_by_id(file_id)?
            .ok_or_else(|| AppError::NotFound(format!("file {file_id}")))?;
        let stored = file.stored_filename.clone();

        self.files.delete(file_id)?;

        if let Err(e) = self.storage.delete(&stored).await
        {
            eprintln!("file deleted in db but not on disk: {e}");
        }
        Ok(())
    }
}

/// Force-suppress all files and the order row itself. Used when an
/// upload sequence fails partway through, so the half-created order
/// does not leak into the admin view.
pub struct CancelOrderUseCase<O, F>
where
    O: OrderRepository,
    F: OrderFileRepository,
{
    orders: Arc<O>,
    files: Arc<F>,
    storage: Arc<dyn FileStorage>,
}

impl<O, F> CancelOrderUseCase<O, F>
where
    O: OrderRepository,
    F: OrderFileRepository,
{
    pub fn new(orders: Arc<O>, files: Arc<F>, storage: Arc<dyn FileStorage>) -> Self
    {
        Self { orders, files, storage }
    }

    pub async fn execute(&self, order_id: i64) -> Result<(), AppError>
    {
        let files = self.files.find_by_order(order_id).unwrap_or_default();
        self.orders.delete(order_id)?;
        for f in &files
        {
            let _ = self.storage.delete(&f.stored_filename).await;
        }
        Ok(())
    }
}

/// Remove every file attached to an order from disk and from the DB,
/// but leave the order row itself untouched. Called automatically when
/// an order transitions to a terminal state (`livre`/`annule`) so that
/// completed orders do not occupy disk space forever.
pub struct PurgeOrderFilesUseCase<F>
where
    F: OrderFileRepository,
{
    files: Arc<F>,
    storage: Arc<dyn FileStorage>,
}

impl<F> PurgeOrderFilesUseCase<F>
where
    F: OrderFileRepository,
{
    pub fn new(files: Arc<F>, storage: Arc<dyn FileStorage>) -> Self
    {
        Self { files, storage }
    }

    pub async fn execute(&self, order_id: i64) -> Result<(), AppError>
    {
        let files = self.files.find_by_order(order_id)?;
        for f in &files
        {
            // Best effort: even if the disk delete fails, remove the
            // DB row so the file is no longer visible. Orphaned bytes
            // can be reclaimed manually.
            let _ = self.files.delete(f.id);
            let _ = self.storage.delete(&f.stored_filename).await;
        }
        Ok(())
    }
}

/// Student-initiated cancellation. Allowed only when the order is
/// still in `a_traiter` (no admin work has started). Transitions the
/// status to `annule` and purges files; the row is kept for history.
pub struct StudentCancelOrderUseCase<O, U, M, F>
where
    O: OrderRepository,
    U: UserRepository,
    M: MaterialRepository,
    F: OrderFileRepository,
{
    orders: Arc<O>,
    purge: Arc<PurgeOrderFilesUseCase<F>>,
    _users: std::marker::PhantomData<U>,
    _materials: std::marker::PhantomData<M>,
}

impl<O, U, M, F> StudentCancelOrderUseCase<O, U, M, F>
where
    O: OrderRepository,
    U: UserRepository,
    M: MaterialRepository,
    F: OrderFileRepository,
{
    pub fn new(orders: Arc<O>, purge: Arc<PurgeOrderFilesUseCase<F>>) -> Self
    {
        Self
        {
            orders,
            purge,
            _users: std::marker::PhantomData,
            _materials: std::marker::PhantomData,
        }
    }

    pub async fn execute(&self, order_id: i64, caller_user_id: i64) -> Result<(), AppError>
    {
        use crate::domain::order::OrderStatus;

        let mut order = self.orders.find_by_id(order_id)?
            .ok_or_else(|| AppError::NotFound(format!("order {order_id}")))?;

        if order.user_id != caller_user_id
        {
            return Err(AppError::NotAuthorized);
        }

        if order.status != OrderStatus::ATraiter
        {
            return Err(AppError::InvalidInput(
                "order can no longer be cancelled".to_owned()));
        }

        order.status = order.status.transition_to(OrderStatus::Annule)?;
        self.orders.update(&order)?;
        self.purge.execute(order_id).await?;
        Ok(())
    }
}

/// Admin-initiated full deletion. Removes the order row entirely
/// (cascade purges file rows in DB) and wipes the on-disk files.
pub struct AdminDeleteOrderUseCase<O, F>
where
    O: OrderRepository,
    F: OrderFileRepository,
{
    cancel: Arc<CancelOrderUseCase<O, F>>,
}

impl<O, F> AdminDeleteOrderUseCase<O, F>
where
    O: OrderRepository,
    F: OrderFileRepository,
{
    pub fn new(cancel: Arc<CancelOrderUseCase<O, F>>) -> Self
    {
        Self { cancel }
    }

    pub async fn execute(&self, order_id: i64, is_admin: bool) -> Result<(), AppError>
    {
        if !is_admin
        {
            return Err(AppError::NotAuthorized);
        }
        self.cancel.execute(order_id).await
    }
}
