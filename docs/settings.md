# Settings

Zako3 has three kind of settings.

- User Settings: These settings are user-specific and can be configured by individual users.
- Guild Settings: These settings are guild-specific and can be configured by guild administrators.
- Admin Settings: These settings are global and only for admin.

## Keys
Each kind of settings has its own set of keys. Keys has the following attributes:
- **Identifier** A unique identifier for the setting. It has three attributes separated by dots: `<tab>.<category>.<name>`.
- **Friendly Name** A human-readable name for the setting
- **Type** The data type of the setting
- **Default Value** The default value of the setting. It's used in **GLOBAL** scope
- **Description** A description of the setting
- **Precedence Merging** Whether precedence merging is enabled. Can only be used on types that implements a merging trait.
- **Admin Only** A boolean indicating whether the setting is only manipulable by admins

## Entries
- **Key Identifier** An identifier of settings key
- **Value**
- **Is Important** Boolean which indicates whether the value is important or not
- **Scope** The scope

User settings also have an additional attribute:
- Allowed Scopes: A list of scopes where the setting can be applied. (default: all)

## Types of Values
Enum-based types implement trait that shows description of each variant.
### Primitives
- `Boolean`
- `Integer(Range)`
- `String(Pattern)`
- `SomeOrDefault<T>`

### Specials
- `VoiceChannelFollowingRule`: one of
    - `Manual`: Users control the bot's channel by command.
    - `Follow Non-empty Channel`: Follow if there's non-empty voice channel, if the bot is not currently being used in the guild.
- `MemberFilter`: one of
    - `Anyone`
    - `WithPermission(Permission Flags)`: Member with specific permissions
- `MappingConfig`: (Precedence Merging Enabled) all of
    - `TextMappingConfig`: array of one of
        - `SimpleTextMapping`: all of
            - `from`
            - `to`
        - `RegexTextMapping`: all of
            - `fromRegex`
            - `replaceTo`
    - `EmojiMappingConfig`: array of all of
        - `emoji`
        - `text or off`
    - `StickerMappingConfig`: array of all of
        - `sticker`
        - `text or off`
- `TapRef`: Simple reference to a tap

## Scope System
Each kind of settings has their own set of scopes. Scopes are used to implement cascading default-value of the settings.

### List of Scopes
#### User Settings
Following is the list of the scopes. It's sorted in ascending order according to priority. So latter ones win.

| Name | Writable by | Description |
|------|-------------|-------------|
| **GLOBAL** | Admin | Settings global to the entire settings system. This is meta-scope, which is derived from default values rather than deriving from a DB |
| **GUILD** | Admin + Guild Admin | Settings local to all users in a guild |
| **USER** | Admin + User | Settings local to a specific user |
| **PER_GUILD_USER** | Admin + User + Guild Admin | Settings that apply when a specific user is in a specific guild |

#### Guild Settings
| Name | Writable by | Description |
|------|-------------|-------------|
| **GLOBAL** | Admin | Settings global to the entire settings system. This is meta-scope, which is derived from default values rather than deriving from a DB |
| **GUILD** | Admin + Guild Admin | Settings local to all users in a guild |

#### Admin Settings
| Name | Writable by | Description |
|------|-------------|-------------|
| **ADMIN** | Admin | The only scope |

### Important Entry
Important entry makes a specific entry, even prioritizing itself over scope with higher priority. It maintains simplicity by inverting the priorities of scopes. So for an important entry, an entry with a scope with lower priority in non-important mode is prioritized over a scope with higher priority in non-important mode. i.e. A value in **GUILD** scope with an important flag wins a value in **USER** scope.

If specific key has mixed important/non-important entries per scope, all non-important entries are ignored.

### Precedence Merging
Precedence merging allows a value with multiple items inside to be merged. For example, `MappingConfig` of all scopes are merged and evaluated in order of entry precednece.

## List of Keys
### User Settings
| Name | Type | Description | Default | Notes |
|------|------|-------------|---------|-------|
| Mappings | `MappingConfig` | Configuration about mappings. | Empty | x |
| Read Text Even Not In Voice Channel | `Boolean` | The name explains. | True | x |
| TTS Voice | `TapRef` | Selected TTS Tap. | `google` | x |
| User Join Alert Name | `SomeOrDefault<String>` | Name of the user to use on the join alert | Member nickname or username | x |
| Stop TTS Keywords | `List<String>` | List of keywords to stop its TTS | 닥쳐 | x |
| Allow Volume Over Limit | `Boolean` | Whether allow the user to set volume over 100% | False | Admin Only |

### Guild Settings
| Name | Type | Description | Default |
|------|------|-------------|---------|
| Voice Channel Following Rule | `VoiceChannelFollowingRule` | Chooses how the bot follows VC | Manual |
| Join Leave Command Permission | `MemberFilter` | Select who can use `/join` and `/leave` | Anyone |
| Enable Disable Command Permission | `MemberFilter` | Select who can use `/tts-channel enable` and `/tts-channel disable` | Anyone |
| Can User Make Bot Join Channel Without Permission | `Boolean` | Whether a user can use `/join` or `/leave`, `/tts-channel` for channels that the user doesn't have access to | False |

### Admin Settings
| Name | Type | Description | Default |
|------|------|-------------|---------|
| Admins | `List<UserId>` | List of admins | Empty |
