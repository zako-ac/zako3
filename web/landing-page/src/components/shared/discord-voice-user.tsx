import Image from "next/image"

interface DiscordVoiceUserProps {
    name: string
    avatarSrc: string
    tag?: string
    isSpeaking?: boolean
}

export function DiscordVoiceUser({ name, avatarSrc, tag, isSpeaking }: DiscordVoiceUserProps) {
    return (
        <div className="flex items-center gap-2 px-2 py-1 rounded mx-1 hover:bg-discord-4/60 transition-colors cursor-pointer group">
            <div className="relative flex-shrink-0">
                <Image
                    src={avatarSrc}
                    alt={name}
                    width={24}
                    height={24}
                    className={`rounded-full object-cover ${isSpeaking ? "ring-2 ring-green-500" : ""}`}
                />
                <span className="absolute -bottom-0.5 -right-0.5 w-3 h-3 bg-green-500 rounded-full border-2 border-discord-2" />
            </div>
            <span className="text-sm text-[#dcddde] group-hover:text-white transition-colors">{name}</span>
            {tag && <span className="text-xs text-[#8e9297] ml-0.5">{tag}</span>}
        </div>
    )
}
