use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, zod_gen_derive::ZodSchema)]
pub enum Permission {
    ManageTapVerifications,
    ManageGlobalMappings,
}
