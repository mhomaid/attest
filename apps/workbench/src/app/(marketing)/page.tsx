import { AgentGuardsSection } from "@/components/marketing/agent-guards-section";
import { CtaSection } from "@/components/marketing/cta-section";
import { DetectionsCatalogSection } from "@/components/marketing/detections-catalog-section";
import { HeroSection } from "@/components/marketing/hero-section";
import { MarketingFooter } from "@/components/marketing/marketing-footer";
import { MarketingNav } from "@/components/marketing/marketing-nav";
import { PipelineSection } from "@/components/marketing/pipeline-section";
import { WorkbenchTourSection } from "@/components/marketing/workbench-tour-section";

export default function HomePage() {
  return (
    <>
      <MarketingNav />
      <main>
        <HeroSection />
        <PipelineSection />
        <AgentGuardsSection />
        <DetectionsCatalogSection />
        <WorkbenchTourSection />
        <CtaSection />
      </main>
      <MarketingFooter />
    </>
  );
}
