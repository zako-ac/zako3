use bitflags::bitflags;

bitflags! {
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct PermissionFlags: u32 {
        const BaseUser = 0b00000001;
        const ManageIdentity = 0b00000010;
    }
}
