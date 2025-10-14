use zako3_hq_server::{
    feature::user::{
        User,
        repository::{UpdateUser, UserRepository},
    },
    util::{permission::PermissionFlags, snowflake::Snowflake},
};

use crate::common::db::create_postgres_test;

mod common;

#[tokio::test]
async fn test_user_db() {
    let (_guard, db) = create_postgres_test().await;

    let id = Snowflake::new_now();
    let perm = PermissionFlags::all();

    let ident = User {
        id: id.as_lazy(),
        name: Some("hi".into()),
        permissions: perm.clone(),
    };

    {
        db.create_user(&ident).await.unwrap();
        let ident_found = db.find_user(id.as_lazy()).await.unwrap().unwrap();
        assert_eq!(ident_found, ident);
    }

    {
        db.update_user(
            id.as_lazy(),
            &UpdateUser {
                permissions: Some(PermissionFlags::empty()),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        let ident_found = db.find_user(id.as_lazy()).await.unwrap().unwrap();

        assert_eq!(ident_found.name, Some("hi".to_string()));
        assert_eq!(ident_found.permissions, PermissionFlags::empty());
    }

    {
        db.delete_user(id.as_lazy()).await.unwrap();
        let ident_found = db.find_user(id.as_lazy()).await.unwrap();
        assert_eq!(ident_found, None);
    }
}
