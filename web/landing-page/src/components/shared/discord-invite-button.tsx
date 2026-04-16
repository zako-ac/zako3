"use client"

import { useState, useRef, useEffect } from "react"
import { ChevronRight } from "lucide-react"
import { Button } from "@/components/ui/button"
import { Card } from "@/components/ui/card"
import { DiscordIcon } from "@/components/shared/discord-icon"
import { cn } from "@/lib/utils"

interface DiscordInviteButtonProps {
    label: string
    className?: string
}

export function DiscordInviteButton({ label, className }: DiscordInviteButtonProps) {
    const [open, setOpen] = useState(false)
    const ref = useRef<HTMLDivElement>(null)

    const mainInvite = process.env.NEXT_PUBLIC_DISCORD_INVITE ?? "#"
    const extraInvitesRaw = process.env.NEXT_PUBLIC_EXTRA_INVITES ?? ""
    const extraInvites = extraInvitesRaw.split(",").map((s) => s.trim()).filter(Boolean)
    const invites = [mainInvite, ...extraInvites]

    useEffect(() => {
        const handleClickOutside = (e: MouseEvent) => {
            if (ref.current && !ref.current.contains(e.target as Node)) {
                setOpen(false)
            }
        }
        document.addEventListener("mousedown", handleClickOutside)
        return () => document.removeEventListener("mousedown", handleClickOutside)
    }, [])

    const getInviteLabel = (index: number) => {
        if (index === 0) return "메인 봇 초대"
        return `서브 봇 ${index} 초대`
    }

    return (
        <div ref={ref} className="relative">
            <Button
                size="lg"
                className={cn("gap-2 text-base pl-6 pr-8 py-6", className)}
                onClick={() => setOpen((prev) => !prev)}
            >
                <ChevronRight
                    className={cn("h-4 w-4 shrink-0 transition-transform duration-200", open && "rotate-90")}
                />
                <DiscordIcon className="h-5 w-5" />
                {label}
            </Button>

            {open && (
                <Card className="absolute left-0 top-full z-50 mt-2 min-w-48 overflow-hidden p-1 shadow-lg border-2 border-border">
                    <div className="w-full flex pl-2 pt-2 font-bold text-muted-foreground">초대할 봇 선택</div>
                    {invites.map((invite, index) => (
                        <a
                            key={index}
                            href={invite}
                            target="_blank"
                            rel="noopener noreferrer"
                            className="flex w-full items-center gap-2 rounded-md px-3 py-2 text-sm transition-colors hover:bg-primary/30 hover:text-accent-foreground"
                            onClick={() => setOpen(false)}
                        >
                            <DiscordIcon className="h-4 w-4 shrink-0" />
                            {getInviteLabel(index)}
                            <ChevronRight className="h-4 w-4 shrink-0 text-muted-foreground ml-auto" />
                        </a>
                    ))}
                </Card>
            )}
        </div>
    )
}
