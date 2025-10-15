use bitflags::bitflags;
use serde::{Deserialize, Serialize};

bitflags! {
    #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
    pub struct PermissionFlags: u32 {
        const BaseUser = 0b00000001;
        const Admin = 0b00000010;
    }
}
