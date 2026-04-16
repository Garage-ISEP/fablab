use std::sync::Arc;

use crate::application::errors::AppError;
use crate::domain::notifications::{NotifiedFile, OrderNotification};
use crate::domain::order_file::OrderFile;
use crate::domain::repositories::
{
    MaterialRepository, NotificationSender, OrderFileRepository, OrderRepository,
    UserRepository,
};

pub struct NotifyNewOrderUseCase<O, U, M, OF, N>
where
    O: OrderRepository,
    U: UserRepository,
    M: MaterialRepository,
    OF: OrderFileRepository,
    N: NotificationSender,
{
    orders: Arc<O>,
    users: Arc<U>,
    materials: Arc<M>,
    files: Arc<OF>,
    notifier: Arc<N>,
    download_base_url: Arc<str>,
}

impl<O, U, M, OF, N> NotifyNewOrderUseCase<O, U, M, OF, N>
where
    O: OrderRepository,
    U: UserRepository,
    M: MaterialRepository,
    OF: OrderFileRepository,
    N: NotificationSender,
{
    pub fn new(
        orders: Arc<O>,
        users: Arc<U>,
        materials: Arc<M>,
        files: Arc<OF>,
        notifier: Arc<N>,
        download_base_url: Arc<str>,
    ) -> Self
    {
        Self { orders, users, materials, files, notifier, download_base_url }
    }

    pub fn execute(&self, order_id: i64) -> Result<(), AppError>
    {
        let order = self.orders.find_by_id(order_id)?
            .ok_or_else(|| AppError::NotFound(format!("order {order_id}")))?;
        let user = self.users.find_by_id(order.user_id)?
            .ok_or_else(|| AppError::NotFound(format!("user {}", order.user_id)))?;
        let material_label = match order.material_id
        {
            Some(mid) => self.materials.find_by_id(mid)?.map(|m| m.label()),
            None => None,
        };
        let file_rows: Vec<OrderFile> = self.files.find_by_order(order.id)?;
        let notified: Vec<NotifiedFile> = file_rows
            .iter()
            .map(|f| NotifiedFile
            {
                file_id: f.id,
                original_filename: f.original_filename.clone(),
                size_bytes: f.size_bytes,
            })
            .collect();

        let n = OrderNotification
        {
            order_id: order.id,
            created_at: order.created_at.clone(),
            user_display_name: user.display_name.clone(),
            user_email: user.email.clone(),
            user_phone: user.phone.clone(),
            software_used: order.software_used.clone(),
            material_label,
            quantity: order.quantity,
            comments: order.comments.clone(),
            files: notified,
            download_base_url: self.download_base_url.as_ref().to_owned(),
        };

        self.notifier.notify_new_order(&n)?;
        Ok(())
    }
}
