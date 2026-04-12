"use client"

import Image from "next/image"
import { useState, useEffect } from "react"
import { Volume2, Radio, Mic, Wand2 } from "lucide-react"
import { Card } from "@/components/ui/card"
import { useIsMobile } from "@/hooks/use-mobile"

const ttsOptions = [
    { id: "google", name: "Google TTS", icon: "🌐", Icon: Radio, description: "표준 음성 합성" },
    { id: "naver", name: "Naver TTS", icon: "🟢", Icon: Volume2, description: "한국어 최적화" },
    { id: "sam", name: "SAM TTS", icon: "🎙️", Icon: Mic, description: "고품질 음성" },
    { id: "custom", name: "User-made TTS", icon: "⚡", Icon: Wand2, description: "커스텀 음성" },
]

export function DemoSection() {
    const [selectedSource, setSelectedSource] = useState(0)
    const isMobile = useIsMobile()

    useEffect(() => {
        const interval = setInterval(() => {
            setSelectedSource((prev) => (prev + 1) % ttsOptions.length)
        }, 2500)
        return () => clearInterval(interval)
    }, [])

    const radius = isMobile ? 140 : 200

    return (
        <section className="px-4 py-20 md:py-32">
            <div className="max-w-6xl mx-auto">
                <div className="text-center mb-16 space-y-4">
                    <h2 className="text-4xl md:text-5xl font-bold text-balance">간편한 TTS 시스템</h2>
                    <p className="text-lg text-muted-foreground max-w-2xl mx-auto text-balance leading-relaxed">
                        다양한 TTS 엔진을 하나의 명령어로 쉽게 사용하세요
                    </p>
                </div>

                <Card className="p-8 md:p-12 bg-card/50 backdrop-blur border-border/50 shadow-2xl">
                    <div className="space-y-12">
                        {/* Discord Command UI */}
                        <div className="bg-discord-1 rounded-lg px-5 py-3 font-mono text-sm border-discord-3 border-2">
                            <div className="flex items-center gap-3 flex-wrap">
                                <img
                                    className="w-8 h-8 rounded-full"
                                    src="/assets/zakopsa.png"
                                    alt="zakopsa"
                                    width={32}
                                    height={32}
                                />
                                <span className="text-foreground font-semibold">/재생</span>
                                <div className="flex items-center rounded-md border-primary border-2">
                                    <div className="flex p-2 items-center justify-center bg-discord-2 rounded-l-md">소스</div>
                                    <div className="flex p-2 items-center justify-center bg-discord-3 rounded-r-md">
                                        {ttsOptions[selectedSource].name}
                                    </div>
                                </div>
                                <div className="flex items-center rounded-md border-discord-4 border-2">
                                    <div className="flex p-2 items-center justify-center bg-discord-2 rounded-l-md">내용</div>
                                    <div className="flex p-2 items-center justify-center bg-discord-3 rounded-r-md">Howdy!</div>
                                </div>
                            </div>
                        </div>

                        {/* Interactive Demo */}
                        <div className="relative py-32 md:py-40">
                            <div className="absolute inset-0 bg-gradient-to-b from-primary/5 via-transparent to-transparent rounded-2xl" />
                            <div className="absolute top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 w-96 h-96 bg-primary/5 rounded-full blur-3xl" />

                            <div className="relative flex items-center justify-center">
                                <div className="relative z-10">
                                    <div className="w-40 h-40 md:w-48 md:h-48 rounded-full bg-primary flex items-center justify-center">
                                        <Image
                                            src="/assets/zakopsa.png"
                                            alt="Zakopsa Avatar"
                                            className="absolute w-36 h-36 md:w-44 md:h-44 rounded-full object-cover"
                                            width={176}
                                            height={176}
                                        />
                                    </div>
                                    <div className="absolute inset-0 rounded-full border-4 border-primary animate-ping" />
                                </div>
                            </div>

                            <div className="absolute inset-0 flex items-center justify-center">
                                <div className="relative w-full max-w-3xl h-[500px] md:h-[600px]">
                                    {ttsOptions.map((option, index) => {
                                        const angle = (index * Math.PI * 2) / ttsOptions.length - Math.PI / 2
                                        const x = Math.cos(angle) * radius
                                        const y = Math.sin(angle) * radius
                                        const isSelected = selectedSource === index

                                        return (
                                            <button
                                                key={option.id}
                                                onClick={() => setSelectedSource(index)}
                                                className={`absolute top-1/2 left-1/2 w-20 h-20 md:w-24 md:h-24 rounded-2xl flex flex-col items-center justify-center gap-1 transition-all duration-500 cursor-pointer border-2 ${isSelected
                                                        ? "bg-secondary border-muted-foreground shadow-xl ring-4 ring-muted-foreground/20"
                                                        : "bg-secondary/50 border-border hover:bg-secondary hover:border-muted-foreground/50"
                                                    }`}
                                                style={{
                                                    transform: `translate(calc(-50% + ${x}px), calc(-50% + ${y}px)) scale(${isSelected ? 1.1 : 0.9})`,
                                                }}
                                            >
                                                <option.Icon
                                                    className={`h-6 w-6 md:h-8 md:w-8 ${isSelected ? "text-primary" : "text-muted-foreground"} transition-colors`}
                                                />
                                                <span className="text-xs font-medium text-muted-foreground">{option.id.toUpperCase()}</span>
                                            </button>
                                        )
                                    })}
                                </div>
                            </div>
                        </div>

                        {/* TTS Option Cards */}
                        <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
                            {ttsOptions.map((option, index) => (
                                <button
                                    key={option.id}
                                    onClick={() => setSelectedSource(index)}
                                    className={`p-4 rounded-lg border transition-all group ${selectedSource === index
                                            ? "border-muted-foreground bg-secondary shadow-lg"
                                            : "border-border bg-card hover:border-muted-foreground/50 hover:bg-secondary/50"
                                        }`}
                                >
                                    <div className="flex items-center gap-3 mb-2">
                                        <div
                                            className={`w-10 h-10 rounded-lg flex items-center justify-center transition-colors ${selectedSource === index ? "bg-primary/10" : "bg-secondary"
                                                }`}
                                        >
                                            <option.Icon
                                                className={`h-5 w-5 ${selectedSource === index ? "text-primary" : "text-muted-foreground"}`}
                                            />
                                        </div>
                                        <div className="text-xl">{option.icon}</div>
                                    </div>
                                    <div className="text-sm font-medium text-left">{option.name}</div>
                                    <div
                                        className={`text-xs text-left mt-1 transition-opacity ${selectedSource === index
                                                ? "text-muted-foreground opacity-100"
                                                : "text-muted-foreground/50 opacity-30 group-hover:opacity-100"
                                            }`}
                                    >
                                        {option.description}
                                    </div>
                                </button>
                            ))}
                        </div>
                    </div>
                </Card>
            </div>
        </section>
    )
}
