pub(super) mod read;
pub(super) mod write;

pub use read::FileContent;

#[macro_export]
macro_rules! signal {
    ($self:ident, $uuid:expr, $action:expr) => {
        if $self.sync.send(($uuid.to_string(), $action)).await.is_err() {
            error!("Cache is shutting down!");
        }
    };
}
