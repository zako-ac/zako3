use zako3_hq_server::{
    feature::identity::{
        entity::Identity,
        repository::{IdentityRepository, UpdateIdentity},
    },
    util::{permission::PermissionFlags, snowflake::Snowflake},
};

use crate::db::create_postgres_test;

mod db;

#[tokio::test]
async fn test_identity_db() {
    let (_guard, db) = create_postgres_test().await;

    let id = Snowflake::new_now(42);
    let perm = PermissionFlags::all();

    let ident = Identity {
        id: id.as_lazy(),
        name: Some("hi".into()),
        permissions: perm.clone(),
    };

    {
        db.create_identity(&ident).await.unwrap();
        let ident_found = db.find_identity(id.as_lazy()).await.unwrap().unwrap();
        assert_eq!(ident_found, ident);
    }

    {
        db.update_identity(
            id.as_lazy(),
            &UpdateIdentity {
                permissions: Some(PermissionFlags::empty()),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        let ident_found = db.find_identity(id.as_lazy()).await.unwrap().unwrap();

        assert_eq!(ident_found.name, Some("hi".to_string()));
        assert_eq!(ident_found.permissions, PermissionFlags::empty());
    }

    {
        db.delete_identity(id.as_lazy()).await.unwrap();
        let ident_found = db.find_identity(id.as_lazy()).await.unwrap();
        assert_eq!(ident_found, None);
    }
}
