import { AgentsSection } from "@/components/marketing/agents-section";
import { ArchitectureSection } from "@/components/marketing/architecture-section";
import { CtaSection } from "@/components/marketing/cta-section";
import { DetectionsCatalogSection } from "@/components/marketing/detections-catalog-section";
import { HeroSection } from "@/components/marketing/hero-section";
import { LandscapeSection } from "@/components/marketing/landscape-section";
import { MarketingFooter } from "@/components/marketing/marketing-footer";
import { MarketingNav } from "@/components/marketing/marketing-nav";
import { PhasesSection } from "@/components/marketing/phases-section";
import {
  FabricSection,
  PillarsBento,
} from "@/components/marketing/pillars-bento";
import { PrinciplesSection } from "@/components/marketing/principles-section";
import { ShippedStatusSection } from "@/components/marketing/shipped-status-section";
import { SubstrateSection } from "@/components/marketing/substrate-section";
import { UseCasesSection } from "@/components/marketing/use-cases-section";
import { WorkbenchTourSection } from "@/components/marketing/workbench-tour-section";

export default function HomePage() {
  return (
    <>
      <MarketingNav />
      <main>
        <HeroSection />
        <LandscapeSection />
        <ShippedStatusSection />
        <ArchitectureSection />
        <PrinciplesSection />
        <PhasesSection />
        <PillarsBento />
        <AgentsSection />
        <DetectionsCatalogSection />
        <FabricSection />
        <SubstrateSection />
        <WorkbenchTourSection />
        <UseCasesSection />
        <CtaSection />
      </main>
      <MarketingFooter />
    </>
  );
}
