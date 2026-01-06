import type { GuildId, Session } from "./types.js";

export interface SessionManager {
    getSession(guildId: GuildId): Promise<Session | null>;
    saveSession(session: Session): Promise<void>;
    modifySession(guildId: GuildId, modify: (session: Session) => Session): Promise<void>;

    destroySession(guildId: GuildId): Promise<void>;
}
