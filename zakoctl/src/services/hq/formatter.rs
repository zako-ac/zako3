use comfy_table::{Attribute, Cell, Color, Table};
use zako3_types::hq::{Tap, User};

pub fn format_user_list(users: &[User]) -> String {
    let mut table = Table::new();
    table.set_header(vec![
        Cell::new("ID")
            .add_attribute(Attribute::Bold)
            .fg(Color::Cyan),
        Cell::new("Username").add_attribute(Attribute::Bold),
        Cell::new("Discord ID").add_attribute(Attribute::Bold),
        Cell::new("Permissions").add_attribute(Attribute::Bold),
        Cell::new("Created At").add_attribute(Attribute::Bold),
    ]);

    for user in users {
        table.add_row(vec![
            user.id.0.to_string(),
            user.username.0.clone(),
            user.discord_user_id.0.clone(),
            user.permissions.join(", "),
            user.timestamp.created_at.to_rfc3339(),
        ]);
    }

    table.to_string()
}

pub fn format_user_details(user: &User) -> String {
    let mut table = Table::new();
    table.add_row(vec!["ID", &user.id.0.to_string()]);
    table.add_row(vec!["Username", &user.username.0]);
    table.add_row(vec!["Discord ID", &user.discord_user_id.0]);
    table.add_row(vec!["Email", user.email.as_deref().unwrap_or("-")]);
    table.add_row(vec!["Permissions", &user.permissions.join(", ")]);
    table.add_row(vec!["Created At", &user.timestamp.created_at.to_rfc3339()]);
    table.add_row(vec!["Updated At", &user.timestamp.updated_at.to_rfc3339()]);
    table.to_string()
}

pub fn format_tap_list(taps: &[Tap]) -> String {
    let mut table = Table::new();
    table.set_header(vec![
        Cell::new("ID")
            .add_attribute(Attribute::Bold)
            .fg(Color::Cyan),
        Cell::new("Name").add_attribute(Attribute::Bold),
        Cell::new("Owner ID").add_attribute(Attribute::Bold),
        Cell::new("Occupation").add_attribute(Attribute::Bold),
        Cell::new("Created At").add_attribute(Attribute::Bold),
    ]);

    for tap in taps {
        table.add_row(vec![
            tap.id.0.to_string(),
            tap.name.0.clone(),
            tap.owner_id.0.to_string(),
            format!("{:?}", tap.occupation),
            tap.timestamp.created_at.to_rfc3339(),
        ]);
    }

    table.to_string()
}

pub fn format_tap_details(tap: &Tap) -> String {
    let mut table = Table::new();
    table.add_row(vec!["ID", &tap.id.0.to_string()]);
    table.add_row(vec!["Name", &tap.name.0]);
    table.add_row(vec![
        "Description",
        tap.description.as_deref().unwrap_or("-"),
    ]);
    table.add_row(vec!["Owner ID", &tap.owner_id.0.to_string()]);
    table.add_row(vec!["Occupation", &format!("{:?}", tap.occupation)]);
    table.add_row(vec!["Permission", &format!("{:?}", tap.permission)]);
    table.add_row(vec!["Roles", &format!("{:?}", tap.roles)]);
    table.add_row(vec!["Created At", &tap.timestamp.created_at.to_rfc3339()]);
    table.add_row(vec!["Updated At", &tap.timestamp.updated_at.to_rfc3339()]);
    table.to_string()
}
