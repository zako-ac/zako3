# Discord Bot Command Plan

A refined, structured overview of the bot's command architecture, utilizing subcommands for a cleaner user interface and integrated permission logic.

---

## 🛡️ Channel Management
*Admin-level commands to control bot accessibility.*

| Command | Description | Permission |
| :--- | :--- | :--- |
| `/channel enable` | Allows the bot to be used in the current text channel. | Manage Channels |
| `/channel disable` | Prevents the bot from responding in the current text channel. | Manage Channels |
| `/channel list` | Displays all channels where the bot is currently enabled. | Manage Channels |

---

## 🔊 Voice Connection
*Controlling the bot's presence in Voice Channels (VC).*

* **/join `[channel]`**
    * Joins your current voice channel or a specified one.
* **/leave**
    * Disconnects the bot and clears active queues.
* **/move `<channel>`**
    * Moves the bot to a different voice channel without stopping playback.

---

## 🎵 Music Engine
*Standardized playback and volume control.*

* **/play `<query|url>` `[source]`**
    * Searches and plays audio.
    * **Source options:** `youtube` (default), `spotify`, `soundcloud`.
* **/stop `[scope]`**
    * Stops playback.
    * **Scope:** `current` (default), `queue` (stops and clears list).
* **/skip `[count]`**
    * Skips the current track or a specified number of tracks.
* **/volume `<level>`**
    * Adjusts volume from **0** to **150**.

---

## 📋 Queue Management
*Unified handling for Music and TTS queues with Korean localization.*

| Command | Alias (KR) | Description |
| :--- | :--- | :--- |
| `/queue music` | `/대기열 음악` | Displays the current music list with a web dashboard link. |
| `/queue tts` | `/대기열 tts` | Displays the upcoming TTS messages (truncated if long). |
| `/queue web` | `/대기열 열기` | Provides a direct URL to the web-based queue interface. |
| `/clear [type]` | `/클리어` | Clears the queue. Defaults to `music`. |

---

## 🎙️ Text-To-Speech (TTS)
*Advanced TTS with user-specific controls and moderation.*

* **/tts speak `<message>` `[voice]`**
    * Queues a message. Defaults to the user's current voice setting.
* **/tts stop `[target]`**
    * Stops TTS playback.
    * **Logic:** Defaults to `self`. To stop `all` or another `@user`, requires **Mute Members** permission.
* **/tts skip `[target]`**
    * Skips the current TTS message.
    * **Logic:** Defaults to `self`. Skipping others requires **Mute Members** permission.
* **/voice `[provider]`**
    * Changes your personal TTS voice (e.g., Google, Amazon, etc.).

---

## ⚙️ System & Utility

* **/settings**
    * Opens an ephemeral UI to manage personal preferences and bot toggles.
* **/help `[category]`**
    * Shows a categorized help menu (Music, TTS, Admin).

---

### 💡 Pro-Tip for Implementation
> **Localization:** Use Discord's internal localization API so that `/queue` and `/대기열` are recognized as the same command. This keeps your backend clean while supporting multiple languages.
