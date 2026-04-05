import re

def fix_file(path):
    with open(path, "r") as f:
        content = f.read()

    # Add channel_id field to commands except Join (already has it)
    commands = ["Leave", "Play", "SetVolume", "Stop", "StopMany", "NextMusic", "GetSessionState"]
    
    for cmd in commands:
        pattern = r"(\s+)(\#\[arg\(short = 'g', long, help = \"The Guild ID\"\)\]\s+guild_id: Option<String>,)"
        
        replacement = r"\1\2\1#[arg(short = 'c', long, help = \"The Channel ID\")]\1channel_id: Option<String>,"
        
        content = re.sub(pattern, replacement, content)

    with open(path, "w") as f:
        f.write(content)

fix_file("zakoctl/src/services/audio_engine/cli.rs")
