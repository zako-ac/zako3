"use client"

import { useState } from "react"
import { Button } from "@/components/ui/button"
import { Badge } from "@/components/ui/badge"
import { SettingsMenu } from "@/components/settings-menu"
import { DiscordInviteButton } from "@/components/shared/discord-invite-button"

const translations = {
    ko: {
        hero: "ZAKO",
        tagline: "통화 채널을 더 다채롭게",
        description: "마이크 없이 음성 채널에서 활동하는 유저들을 위한 강력한 TTS 및 미디어 봇 (베타)",
        addBot: "디스코드에 추가하기",
        documentation: "문서 보기",
        stats1: "1K+",
        stats1Label: "오디오 데이터",
        stats2: "10K+",
        stats2Label: "월간 메시지",
        stats3: "99.9%",
        stats3Label: "가동시간",
        available: "지금 사용 가능",
        dashboard: "대시보드",
        usage: "사용법",
    },
    en: {
        hero: "ZAKO",
        tagline: "Make Voice Channels More Colorful",
        description: "A powerful TTS and media bot for users active in voice channels without a microphone",
        addBot: "Add to Discord",
        documentation: "Documentation",
        stats1: "1K+",
        stats1Label: "Audio Data",
        stats2: "10K+",
        stats2Label: "Monthly Messages",
        stats3: "99.9%",
        stats3Label: "Uptime",
        available: "Now Available",
        dashboard: "Dashboard",
        usage: "Usage",
    },
}

export function HeroSection() {
    const [lang, setLang] = useState<"ko" | "en">("ko")
    const t = translations[lang]

    return (
        <section className="relative min-h-screen flex flex-col items-center justify-center px-4 py-20 overflow-hidden">
            <div className="absolute inset-0 -z-10">
                <div className="absolute top-1/4 left-1/4 w-96 h-96 bg-primary/20 rounded-full blur-3xl" />
                <div className="absolute bottom-1/4 right-1/4 w-96 h-96 bg-primary/10 rounded-full blur-3xl" />
            </div>

            <div className="absolute top-8 right-8">
                <SettingsMenu lang={lang} onLanguageChange={setLang} />
            </div>

            <div className="max-w-6xl mx-auto text-center space-y-8">
                <Badge variant="secondary" className="px-4 py-3 text-sm font-medium">
                    <span className="inline-flex items-center gap-2">
                        <span className="relative flex h-2 w-2">
                            <span className="animate-ping absolute inline-flex h-full w-full rounded-full bg-primary opacity-75" />
                            <span className="relative inline-flex rounded-full h-2 w-2 bg-primary" />
                        </span>
                        {t.available}
                    </span>
                </Badge>

                <h1 className="text-8xl md:text-9xl lg:text-[12rem] font-light tracking-tight text-balance leading-none hero">
                    {t.hero}
                </h1>

                <p className="text-2xl md:text-3xl lg:text-4xl font-medium text-balance max-w-3xl mx-auto bg-linear-to-r from-foreground to-muted-foreground bg-clip-text text-transparent">
                    {t.tagline}
                </p>

                <p className="text-lg md:text-xl text-muted-foreground max-w-2xl mx-auto leading-relaxed text-balance">
                    {t.description}
                </p>

                <div className="flex flex-wrap flex-col items-center justify-center gap-4 pt-4">
                    <DiscordInviteButton
                        label={t.addBot}
                        className="bg-primary hover:bg-primary/90 shadow-lg shadow-primary/25"
                    />
                    <div className="flex gap-4">
                        <Button asChild size="lg" variant="secondary" className="gap-2 text-base px-8 py-6">
                            <a href='/dashboard' target="_blank" rel="noopener noreferrer">
                                {t.dashboard}
                            </a>
                        </Button>
                        <Button asChild size="lg" variant="secondary" className="gap-2 text-base px-8 py-6">
                            <a href={process.env.NEXT_PUBLIC_DOCS_URL} target="_blank" rel="noopener noreferrer">
                                {t.usage}
                            </a>
                        </Button>
                    </div>
                </div>

                <div className="grid grid-cols-1 sm:grid-cols-2 gap-8 pt-12 max-w-3xl mx-auto">
                    <div className="space-y-2">
                        <div className="text-4xl font-bold text-primary">{t.stats1}</div>
                        <div className="text-sm text-muted-foreground">{t.stats1Label}</div>
                    </div>
                    <div className="space-y-2">
                        <div className="text-4xl font-bold text-primary">{t.stats2}</div>
                        <div className="text-sm text-muted-foreground">{t.stats2Label}</div>
                    </div>
                </div>
            </div>
        </section>
    )
}
