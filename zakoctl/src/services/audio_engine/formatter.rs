use colored::*;
use comfy_table::{Attribute, Cell, ContentArrangement, Table, presets::UTF8_FULL};
use zako3_audio_engine_protos::{
    OkResponse, PlayResponse, SessionStateResponse, ok_response, play_response,
    session_state_response,
};

pub fn print_ok(label: &str, response: OkResponse) {
    match response.result {
        Some(ok_response::Result::Success(true)) => {
            println!("{} {}", label.bold(), "Success".green());
        }
        Some(ok_response::Result::Success(false)) => {
            println!("{} {}", label.bold(), "Failed (False)".red());
        }
        Some(ok_response::Result::Error(e)) => {
            println!("{} {} - {}", label.bold(), "Failed".red(), e.message);
        }
        None => {
            println!("{} {}", label.bold(), "Unknown response state".yellow());
        }
    }
}

pub fn print_play(response: PlayResponse) {
    println!("{}", "Play Request".cyan().bold().underline());
    match response.result {
        Some(play_response::Result::TrackId(id)) => {
            println!(
                "{} {}",
                "Track Enqueued with ID:".green(),
                id.to_string().bold()
            );
        }
        Some(play_response::Result::Error(e)) => {
            println!("{} {}", "Error:".red().bold(), e.message);
        }
        None => {
            println!("{}", "Unknown response state".yellow());
        }
    }
}

pub fn print_session_state(response: SessionStateResponse) {
    println!("{}", "Session State".cyan().bold().underline());
    match response.result {
        Some(session_state_response::Result::State(state)) => {
            println!("{}: {}", "Guild ID".blue(), state.guild_id);
            println!("{}: {}", "Channel ID".blue(), state.channel_id);
            println!();

            if state.queues.is_empty() {
                println!("{}", "No active queues.".italic());
                return;
            }

            for queue in state.queues {
                println!("{} {}", "Queue:".purple().bold(), queue.name);

                if queue.tracks.is_empty() {
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
                        Cell::new("Description").add_attribute(Attribute::Bold),
                        Cell::new("Tap").add_attribute(Attribute::Bold),
                        Cell::new("Vol").add_attribute(Attribute::Bold),
                        Cell::new("Request").add_attribute(Attribute::Bold),
                    ]);

                for track in queue.tracks {
                    table.add_row(vec![
                        Cell::new(track.track_id),
                        Cell::new(&track.description),
                        Cell::new(&track.tap_name),
                        Cell::new(format!("{:.1}", track.volume)),
                        Cell::new(&track.audio_request_string),
                    ]);
                }

                println!("{table}");
                println!();
            }
        }
        Some(session_state_response::Result::Error(e)) => {
            println!("{} {}", "Error retrieving state:".red().bold(), e.message);
        }
        None => {
            println!("{}", "Unknown response state".yellow());
        }
    }
}
