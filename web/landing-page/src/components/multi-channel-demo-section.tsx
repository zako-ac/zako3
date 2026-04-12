"use client"

import { Volume2, ChevronDown } from "lucide-react"
import { Card } from "@/components/ui/card"
import { DiscordVoiceUser } from "@/components/shared/discord-voice-user"

const channels = [
    { id: "general", name: "일반", userIndex: 0 },
    { id: "meeting", name: "게임", userIndex: 1 },
]

const users = [
    { id: 1, name: "자코♡", tag: "#1" },
    { id: 2, name: "자코", tag: "#2" },
    { id: 3, name: "자코", tag: "#3" },
    { id: 4, name: "자코", tag: "#4" },
]

export function MultiChannelDemoSection() {
    return (
        <section className="px-4 py-20 md:py-32">
            <div className="max-w-6xl mx-auto">
                <div className="text-center mb-16 space-y-4">
                    <h2 className="text-4xl md:text-5xl font-bold text-balance">여러 통화방 동시 참가</h2>
                    <p className="text-lg text-muted-foreground max-w-2xl mx-auto text-balance leading-relaxed">
                        여러 음성 채널에 동시에 참가하여 모든 대화를 놓치지 마세요
                    </p>
                </div>

                <Card className="p-8 md:p-12 bg-card/50 backdrop-blur border-border/50 shadow-2xl">
                    <div className="flex items-center justify-center">
                        {/* Discord window */}
                        <div className="w-full max-w-sm bg-discord-2 rounded-lg overflow-hidden select-none">
                            {/* Server name header */}
                            <div className="px-4 py-3 bg-discord-1 border-b border-discord-4/50 flex items-center justify-between">
                                <span className="font-semibold text-white text-sm truncate">자코 서버</span>
                                <ChevronDown className="w-4 h-4 text-[#8e9297] flex-shrink-0" />
                            </div>

                            {/* Main area: sidebar + participants */}
                            <div className="flex">
                                {/* Left: channel sidebar */}
                                <div className="w-40 flex-shrink-0 py-3 border-r border-discord-4/30">
                                    <div className="flex items-center gap-1 px-2 mb-1 cursor-pointer group">
                                        <ChevronDown className="w-3 h-3 text-[#8e9297] group-hover:text-[#dcddde] transition-colors" />
                                        <span className="text-xs font-semibold text-[#8e9297] group-hover:text-[#dcddde] uppercase tracking-wide transition-colors">
                                            음성 채널
                                        </span>
                                    </div>
                                    <div className="space-y-0.5">
                                        {channels.map((ch) => (
                                            <div key={ch.id}>
                                                <div className="flex items-center gap-1.5 px-2 py-1 rounded hover:bg-discord-4/60 transition-colors cursor-pointer group mx-1">
                                                    <Volume2 className="w-4 h-4 text-[#8e9297] group-hover:text-[#dcddde] flex-shrink-0" />
                                                    <span className="text-sm text-[#8e9297] group-hover:text-[#dcddde] transition-colors truncate">
                                                        {ch.name}
                                                    </span>
                                                </div>
                                                <DiscordVoiceUser
                                                    name={users[ch.userIndex].name}
                                                    tag={users[ch.userIndex].tag}
                                                    avatarSrc="/assets/zakopsa.png"
                                                    isSpeaking
                                                />
                                            </div>
                                        ))}
                                    </div>
                                </div>

                                {/* Right: participants list */}
                                <div className="flex-1 py-3 px-1">
                                    <div className="px-2 mb-2">
                                        <span className="text-xs font-semibold text-[#8e9297] uppercase tracking-wide">
                                            멤버 — 21
                                        </span>
                                    </div>
                                    <div className="space-y-0.5">
                                        {users.map((user) => (
                                            <DiscordVoiceUser
                                                key={user.id}
                                                name={user.name}
                                                tag={user.tag}
                                                avatarSrc="/assets/zakopsa.png"
                                                isSpeaking
                                            />
                                        ))}
                                    </div>
                                </div>
                            </div>

                            {/* User panel */}
                            <div className="px-2 py-2 bg-discord-1 border-t border-discord-4/50 flex items-center gap-2">
                                <div className="relative">
                                    <img
                                        src="/assets/zakopsa.png"
                                        alt="자코"
                                        className="w-8 h-8 rounded-full object-cover"
                                    />
                                    <span className="absolute -bottom-0.5 -right-0.5 w-3 h-3 bg-green-500 rounded-full border-2 border-discord-1" />
                                </div>
                                <div className="flex-1 min-w-0">
                                    <div className="text-sm font-semibold text-white truncate">자코♡</div>
                                    <div className="text-xs text-[#8e9297] truncate">온라인</div>
                                </div>
                            </div>
                        </div>
                    </div>
                </Card>
            </div>
        </section>
    )
}
