import { Card } from "@/components/ui/card"
import { Mic, Music, Type, PlayCircle, Users, Settings, Zap, Shield } from "lucide-react"

const features = [
  {
    icon: Mic,
    title: "TTS & 음악 재생",
    description: "고품질 텍스트 음성 변환과 음악 스트리밍을 동시에 지원합니다.",
    gradient: "from-pink-500 to-rose-500",
  },
  {
    icon: Users,
    title: "커뮤니티 기반 TTS 음성",
    description: "사용자가 만든 다양한 TTS 음성을 공유하고 사용할 수 있습니다.",
    gradient: "from-purple-500 to-indigo-500",
  },
  {
    icon: Type,
    title: "개인별 텍스트 대치",
    description: "자주 사용하는 단어나 문장을 미리 설정하여 빠르게 재생하세요.",
    gradient: "from-blue-500 to-cyan-500",
  },
  {
    icon: PlayCircle,
    title: "동시 재생",
    description: "여러 채널에서 동시에 음성과 미디어를 재생할 수 있습니다.",
    gradient: "from-emerald-500 to-teal-500",
  },
  {
    icon: Zap,
    title: "빠른 응답 속도",
    description: "최적화된 시스템으로 지연 없는 즉각적인 TTS 재생을 제공합니다.",
    gradient: "from-yellow-500 to-orange-500",
  },
  {
    icon: Shield,
    title: "안정적인 서비스",
    description: "높은 가동률로 언제나 믿을 수 있는 서비스를 제공합니다.",
    gradient: "from-red-500 to-pink-500",
  },
  {
    icon: Settings,
    title: "세밀한 설정",
    description: "음량, 속도, 피치 등 다양한 옵션을 서버별로 커스터마이징하세요.",
    gradient: "from-violet-500 to-purple-500",
  },
  {
    icon: Music,
    title: "다양한 소스 지원",
    description: "사용자가 연동한 여러 플랫폼의 음악을 재생할 수 있습니다.",
    gradient: "from-indigo-500 to-blue-500",
  },
]

export function FeaturesSection() {
  return (
    <section className="px-4 py-20 md:py-32 bg-secondary/30">
      <div className="max-w-7xl mx-auto">
        <div className="text-center mb-16 space-y-4">
          <h2 className="text-4xl md:text-5xl font-bold text-balance">강력한 기능들</h2>
          <p className="text-lg text-muted-foreground max-w-2xl mx-auto text-balance leading-relaxed">
            자코는 음성 채널에서 필요한 모든 기능을 제공합니다
          </p>
        </div>

        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-6">
          {features.map((feature, index) => (
            <Card
              key={index}
              className="group relative overflow-hidden p-6 border-border/50 hover:border-primary/50 transition-all duration-300 hover:shadow-lg hover:shadow-primary/10 bg-card/50 backdrop-blur hover:-translate-y-1"
            >
              <div className="relative z-10 space-y-4">
                <div
                  className={`w-12 h-12 rounded-lg bg-gradient-to-br ${feature.gradient} flex items-center justify-center text-white shadow-lg`}
                >
                  <feature.icon className="h-6 w-6" />
                </div>
                <h3 className="text-xl font-semibold text-balance">{feature.title}</h3>
                <p className="text-muted-foreground leading-relaxed text-balance">{feature.description}</p>
              </div>
              <div className="absolute inset-0 bg-gradient-to-br from-primary/5 to-transparent opacity-0 group-hover:opacity-100 transition-opacity duration-300" />
            </Card>
          ))}
        </div>
      </div>
    </section>
  )
}
