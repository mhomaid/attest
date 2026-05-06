import { CtaSection } from "@/components/marketing/cta-section";
import { HeroSection } from "@/components/marketing/hero-section";
import { LandscapeSection } from "@/components/marketing/landscape-section";
import { MarketingFooter } from "@/components/marketing/marketing-footer";
import { MarketingNav } from "@/components/marketing/marketing-nav";
import { PhasesSection } from "@/components/marketing/phases-section";
import {
  FabricSection,
  PillarsBento,
} from "@/components/marketing/pillars-bento";
import { SubstrateSection } from "@/components/marketing/substrate-section";
import { UseCasesSection } from "@/components/marketing/use-cases-section";

export default function HomePage() {
  return (
    <>
      <MarketingNav />
      <main>
        <HeroSection />
        <LandscapeSection />
        <PhasesSection />
        <PillarsBento />
        <FabricSection />
        <SubstrateSection />
        <UseCasesSection />
        <CtaSection />
      </main>
      <MarketingFooter />
    </>
  );
}
