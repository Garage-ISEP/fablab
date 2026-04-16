#[derive(Debug, Clone)]
pub enum Caller
{
    Admin,
    Student { user_id: i64 },
}

impl Caller
{
    pub fn is_admin(&self) -> bool
    {
        matches!(self, Caller::Admin)
    }

    pub fn student_user_id(&self) -> Option<i64>
    {
        match self
        {
            Caller::Student { user_id } => Some(*user_id),
            Caller::Admin => None,
        }
    }
}
