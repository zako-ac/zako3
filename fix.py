import re

def fix_file(path):
    with open(path, "r") as f:
        content = f.read()

    content = content.replace("                .basic_publish(\n                    \"\",\n                    rt.as_str().into(),", "                .basic_publish(\n                    \"\".into(),\n                    rt.as_str().into(),")

    # Fix Leave logic
    leave_pattern = r"AudioEngineRequest::Leave \{ guild_id, channel_id \} => \{\s*if let Some\(session\) = self\.session_manager\.get_session\(\*guild_id\) \{.*?\} else \{\s*skipped = true;\s*\}\s*\}"
    
    new_leave = """AudioEngineRequest::Leave { guild_id, channel_id } => {
                    if let Some(session) = self.session_manager.get_session(*guild_id) {
                        // Check if we are in the correct channel
                        let state = session.session_state().await;
                        if let Ok(Some(s)) = state {
                            if s.channel_id == *channel_id {
                                match self.session_manager.leave(*guild_id).await {
                                    Ok(_) => {
                                        if let Some((_, handle)) = self.session_consumers.remove(guild_id) {
                                            handle.abort();
                                        }
                                        self.send_reply(&channel, reply_to.clone(), correlation_id.clone(), AudioEngineResponse::SuccessBool(true)).await;
                                    }
                                    Err(e) => {
                                        self.send_reply(&channel, reply_to.clone(), correlation_id.clone(), AudioEngineResponse::Error(e.to_string())).await;
                                    }
                                }
                            } else {
                                skipped = true;
                            }
                        } else {
                            skipped = true;
                        }
                    } else {
                        skipped = true;
                    }
                }"""
                
    content = re.sub(leave_pattern, new_leave, content, flags=re.DOTALL)
    
    with open(path, "w") as f:
        f.write(content)

fix_file("audio-engine/controller/src/server.rs")
