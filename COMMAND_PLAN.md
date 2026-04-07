# Discord Command Plan

- `/channel enable`
- `/channel disable`
- `/channel list`

- `/join <channel>`
- `/leave <channel>`
- `/move <channel1> <channel2>`

- `/play <song> [tap = ytdl]` - YouTube search and play
- `/stop [queue]` - Stop (default: music)
- `/skip [queue]` - Skip current track (default: music)

- `/queue music` - Show music queue
- `/queue web` - Show queue on web interface
- `/clear [queue]` - Clear queue (default: music)

- `/volume [queue]` - Set volume of current track in queue (default: music)

- `/tts [message] [tap]` - Text-to-speech (default tap: user's voice)
- `/tts stop [user]` - Stop TTS for user (default: self), stopping other user requires "mute members" permission
- `/tts skip [user]` - Skip current TTS for user (default: self), skipping other user requires "mute members" permission
- `/tts stop all` - Stop all TTS, requires "mute members" permission
- `/tts queue` - Show TTS queue. Reduce text if long

- `/voice [tap name]` - Change voice (default: google)
- `/settings` - Show settings UI
- `/help` - Show help message
