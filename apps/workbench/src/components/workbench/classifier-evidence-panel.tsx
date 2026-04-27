import type { FeatureImpact } from "@/lib/mock-data";
import { cn } from "@/lib/utils";

export function ClassifierEvidencePanel({ features }: { features: FeatureImpact[] }) {
  const maxImpact = Math.max(...features.map((feature) => Math.abs(feature.impact)));

  return (
    <div className="rounded-lg border border-border bg-card/80 p-3">
      <div className="flex items-center justify-between">
        <h2 className="text-sm font-semibold">Classifier Evidence</h2>
        <span className="font-mono text-[10px] uppercase tracking-[0.18em] text-muted-foreground">
          SHAP-style impact
        </span>
      </div>

      <div className="mt-3 space-y-3">
        {features.map((feature) => {
          const width = `${Math.max(12, (Math.abs(feature.impact) / maxImpact) * 100)}%`;
          const isPositive = feature.impact >= 0;

          return (
            <div key={feature.name}>
              <div className="mb-1 flex items-center justify-between gap-3 text-xs">
                <div>
                  <span className="font-medium">{feature.name}</span>
                  <span className="ml-2 text-muted-foreground">{feature.value}</span>
                </div>
                <span className="font-mono text-muted-foreground">
                  {isPositive ? "+" : ""}
                  {feature.impact.toFixed(2)}
                </span>
              </div>
              <div className="h-2 rounded-full bg-secondary">
                <div
                  className={cn(
                    "h-2 rounded-full",
                    isPositive ? "bg-severity-high" : "bg-signal-good",
                  )}
                  style={{ width }}
                />
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
}
