use colored::*;
use comfy_table::{Attribute, Cell, ContentArrangement, Table, presets::UTF8_FULL};
use zako3_types::SessionState;

pub fn print_session_state_native(state: SessionState) {
    println!("{}", "Session State".cyan().bold().underline());
    println!("{}: {}", "Guild ID".blue(), state.guild_id);
    println!("{}: {}", "Channel ID".blue(), state.channel_id);
    println!();

    if state.queues.is_empty() {
        println!("{}", "No active queues.".italic());
        return;
    }

    for (queue_name, tracks) in state.queues {
        println!("{} {}", "Queue:".purple().bold(), queue_name);

        if tracks.is_empty() {
            println!("  {}", "(Empty)".dimmed());
            println!();
            continue;
        }

        let mut table = Table::new();
        table
            .load_preset(UTF8_FULL)
            .set_content_arrangement(ContentArrangement::Dynamic)
            .set_header(vec![
                Cell::new("ID").add_attribute(Attribute::Bold),
                Cell::new("Tap").add_attribute(Attribute::Bold),
                Cell::new("Vol").add_attribute(Attribute::Bold),
                Cell::new("Request").add_attribute(Attribute::Bold),
            ]);

        for track in tracks {
            table.add_row(vec![
                Cell::new(track.track_id),
                Cell::new(&track.request.tap_name),
                Cell::new(format!("{:.1}", track.volume)),
                Cell::new(&track.request.audio_request),
            ]);
        }

        println!("{table}");
        println!();
    }
}
