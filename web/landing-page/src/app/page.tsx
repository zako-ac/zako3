import { HeroSection } from "@/components/hero-section"
import { DemoSection } from "@/components/demo-section"
import { MultiChannelDemoSection } from "@/components/multi-channel-demo-section"
import { FeaturesSection } from "@/components/features-section"
import { CTASection } from "@/components/cta-section"
import { Footer } from "@/components/footer"

export default function Home() {
  return (
    <main className="min-h-screen">
      <HeroSection />
      <DemoSection />
      <MultiChannelDemoSection />
      <FeaturesSection />
      <CTASection />
      <Footer />
    </main>
  )
}
