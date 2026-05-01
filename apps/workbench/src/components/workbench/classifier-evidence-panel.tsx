"use client";

import type { FeatureImpact } from "@/lib/mock-data";
import { cn } from "@/lib/utils";
import {
  Bar,
  BarChart,
  CartesianGrid,
  Cell,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";

export function ClassifierEvidencePanel({ features }: { features: FeatureImpact[] }) {
  const maxImpact = Math.max(...features.map((f) => Math.abs(f.impact)), 1e-6);
  const data = features.map((f) => ({
    name: f.name,
    displayValue: f.value,
    impact: f.impact,
    fill: f.impact >= 0 ? "hsl(var(--severity-high))" : "hsl(var(--signal-good))",
    absNorm: (Math.abs(f.impact) / maxImpact) * 100,
  }));

  // Empty → triage hasn't returned yet.
  const isEmpty = features.length === 0;
  // All features have negligible impact → model is highly confident this is benign.
  const allNearZero = !isEmpty && features.every((f) => Math.abs(f.impact) < 0.01);

  return (
    <div
      className="rounded-lg border border-border bg-card/80 p-4"
      data-testid="classifier-evidence-panel"
    >
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div>
          <h2 className="text-base font-semibold tracking-tight">Classifier Evidence</h2>
          <p className="mt-1 text-xs leading-5 text-muted-foreground">
            Ranked feature contributions from the ONNX classifier. Red increases risk,
            green reduces it.
          </p>
        </div>
        <span className="rounded-md border border-border bg-background/70 px-2 py-1 font-mono text-[11px] uppercase tracking-[0.16em] text-muted-foreground">
          SHAP impact
        </span>
      </div>

      {isEmpty && (
        <div className="mt-4 rounded-md border border-border/70 bg-background/50 px-3 py-4">
          <p className="text-sm font-medium">Classifier evidence is not available yet.</p>
          <p className="mt-1 text-xs leading-5 text-muted-foreground">
            The panel will populate when the triage response includes classifier input features
            and attribution values.
          </p>
        </div>
      )}

      {allNearZero && (
        <div className="mt-4 flex items-center gap-2 rounded-md border border-signal-good/30 bg-signal-good/5 px-3 py-2 text-xs text-muted-foreground">
          <span className="h-2 w-2 rounded-full bg-signal-good" />
          Attribution values are near zero; showing the input features captured for this run.
          {features[0] && (
            <span className="ml-auto font-mono text-[10px]">
              max Δ {Math.max(...features.map((f) => Math.abs(f.impact))).toFixed(4)}
            </span>
          )}
        </div>
      )}

      {!isEmpty && (
        <ul className="mt-4 space-y-3">
          {features.map((feature) => {
            const width = `${Math.max(8, (Math.abs(feature.impact) / maxImpact) * 100)}%`;
            const isPositive = feature.impact >= 0;
            return (
              <li key={feature.name} data-testid="shap-bar">
                <div className="mb-1.5 flex items-center justify-between gap-4">
                  <div className="min-w-0">
                    <div className="truncate text-sm font-medium capitalize">{feature.name}</div>
                    <div className="font-mono text-[11px] text-muted-foreground">
                      value {feature.value}
                    </div>
                  </div>
                  <div
                    className={cn(
                      "font-mono text-sm font-semibold tabular-nums",
                      isPositive ? "text-severity-high" : "text-signal-good",
                    )}
                  >
                    {feature.impact === 0 ? "0.00" : `${isPositive ? "+" : ""}${feature.impact.toFixed(2)}`}
                  </div>
                </div>
                <div className="h-3 rounded-full bg-secondary/80">
                  <div
                    className={cn(
                      "h-3 rounded-full",
                      isPositive ? "bg-severity-high" : "bg-signal-good",
                      allNearZero && "opacity-50",
                    )}
                    style={{ width }}
                  />
                </div>
              </li>
            );
          })}
        </ul>
      )}

      <div className={cn("mt-5 hidden h-56 w-full md:block", (allNearZero || isEmpty) && "hidden")}>
        <ResponsiveContainer width="100%" height="100%">
          <BarChart data={data} layout="vertical" margin={{ left: 12, right: 20, top: 8, bottom: 8 }}>
            <CartesianGrid strokeDasharray="3 3" className="stroke-border/40" horizontal={false} />
            <XAxis type="number" domain={[0, "auto"]} tick={{ fontSize: 12 }} />
            <YAxis type="category" dataKey="name" width={120} tick={{ fontSize: 12 }} interval={0} />
            <Tooltip
              contentStyle={{ fontSize: 12 }}
              formatter={(v: number) => [`${v >= 0 ? "+" : ""}${Number(v).toFixed(2)}`, "impact"]}
              labelFormatter={(_l, payload) =>
                payload?.[0]?.payload
                  ? `value: ${String((payload[0].payload as { displayValue: string }).displayValue)}`
                  : ""
              }
            />
            <Bar dataKey="absNorm" radius={[0, 4, 4, 0]} isAnimationActive={false}>
              {data.map((entry, i) => (
                <Cell key={i} fill={entry.fill} data-testid="shap-bar" />
              ))}
            </Bar>
          </BarChart>
        </ResponsiveContainer>
      </div>
    </div>
  );
}
