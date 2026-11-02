//! What can be done to a server's content, one operation per file, named as app-lib names the
//! same operation on an instance.

pub mod apply_content_install;
pub mod apply_content_update;
pub mod apply_pending;
pub mod apply_pending_on_stop;
pub mod check_content_updates;
pub mod client_only_files;
pub mod content_mutation;
pub mod drop_own_share;
pub mod edited_files;
pub mod export_mrpack;
pub mod forget_pack;
pub mod install_instance;
pub mod install_loader;
pub mod install_pack;
pub mod list_content;
pub mod sync_content_files;
pub mod write_shared_pack;
