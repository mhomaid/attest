import { AgentGuardsSection } from "@/components/marketing/agent-guards-section";
import { CompareSection } from "@/components/marketing/compare-section";
import { CtaSection } from "@/components/marketing/cta-section";
import { DetectionsCatalogSection } from "@/components/marketing/detections-catalog-section";
import { HeroSection } from "@/components/marketing/hero-section";
import { MarketingFooter } from "@/components/marketing/marketing-footer";
import { MarketingNav } from "@/components/marketing/marketing-nav";
import { PipelineSection } from "@/components/marketing/pipeline-section";
import { QuickstartSection } from "@/components/marketing/quickstart-section";
import { DeploymentSection } from "@/components/marketing/deployment-section";
import { StackDiagramSection } from "@/components/marketing/stack-diagram-section";
import { StatusStrip } from "@/components/marketing/status-strip";
import { TryDemoSection } from "@/components/marketing/try-demo-section";
import { WorkbenchTourSection } from "@/components/marketing/workbench-tour-section";

export default function HomePage() {
  return (
    <>
      <MarketingNav />
      <main>
        <HeroSection />
        <StatusStrip />
        <TryDemoSection />
        <PipelineSection />
        <AgentGuardsSection />
        <CompareSection />
        <StackDiagramSection />
        <DeploymentSection />
        <QuickstartSection />
        <DetectionsCatalogSection />
        <WorkbenchTourSection />
        <CtaSection />
      </main>
      <MarketingFooter />
    </>
  );
}
