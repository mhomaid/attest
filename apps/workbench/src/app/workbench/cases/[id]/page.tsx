import { CaseWorkbench } from "@/components/workbench/case-workbench";
import { getCaseById } from "@/lib/mock-data";

export default async function CasePage({ params }: { params: Promise<{ id: string }> }) {
  const { id } = await params;
  const caseRecord = getCaseById(id);

  return <CaseWorkbench caseRecord={caseRecord} />;
}
