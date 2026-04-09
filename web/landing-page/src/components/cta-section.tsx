import { Button } from "@/components/ui/button"
import { Card } from "@/components/ui/card"
import { ArrowRight } from "lucide-react"
import { DiscordIcon } from "@/components/shared/discord-icon"

export function CTASection() {
    return (
        <section className="px-4 py-20 md:py-32">
            <div className="max-w-4xl mx-auto">
                <Card className="relative overflow-hidden p-12 md:p-16 bg-gradient-to-br from-primary/10 via-primary/5 to-transparent border-primary/20">
                    <div className="absolute top-0 right-0 w-64 h-64 bg-primary/10 rounded-full blur-3xl" />
                    <div className="absolute bottom-0 left-0 w-64 h-64 bg-primary/5 rounded-full blur-3xl" />

                    <div className="relative z-10 text-center space-y-6">
                        <h2 className="text-4xl md:text-5xl font-bold text-balance">지금 바로 시작하세요</h2>
                        <p className="text-lg md:text-xl text-muted-foreground max-w-2xl mx-auto text-balance leading-relaxed">
                            무료로 자코를 디스코드 서버에 추가하고 음성 채널을 더욱 재미있게 만들어보세요
                        </p>
                        <div className="flex flex-col sm:flex-row items-center justify-center gap-4 pt-4">
                            <Button
                                asChild
                                size="lg"
                                className="gap-2 text-base px-8 py-6 bg-primary hover:bg-primary/90 shadow-lg shadow-primary/25 group"
                            >
                                <a href={process.env.NEXT_PUBLIC_DISCORD_INVITE} target="_blank" rel="noopener noreferrer">
                                    <DiscordIcon className="h-5 w-5" />
                                    디스코드에 추가하기
                                    <ArrowRight className="h-4 w-4 group-hover:translate-x-1 transition-transform" />
                                </a>
                            </Button>
                            <Button asChild size="lg" variant="outline" className="gap-2 text-base px-8 py-6">
                                <a href='/dashboard' target="_blank" rel="noopener noreferrer">
                                    대시보드
                                </a>
                            </Button>
                        </div>
                        <p className="text-sm text-muted-foreground pt-4">자코 바보</p>
                    </div>
                </Card>
            </div>
        </section>
    )
}
