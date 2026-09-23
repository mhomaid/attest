"use client";

import { useReducedMotion } from "framer-motion";

type Node = {
  id: string;
  x: number;
  y: number;
  label: string;
  hint: string;
  tone?: "primary" | "good" | "live";
};

const CENTER = 160;

const nodes: Node[] = [
  { id: "src", x: 70, y: CENTER, label: "CloudTrail", hint: "raw JSON" },
  { id: "col", x: 210, y: CENTER, label: "Collector", hint: "OCSF 1.3" },
  { id: "bus", x: 350, y: CENTER, label: "Kafka", hint: "per tenant" },
  { id: "det", x: 490, y: CENTER, label: "Detect", hint: "HELIQL → RW" },
  { id: "cls", x: 640, y: 70, label: "Classifier", hint: "<5 ms ONNX", tone: "good" },
  { id: "llm", x: 640, y: 250, label: "LLM + MCP", hint: "novel only", tone: "live" },
  { id: "env", x: 790, y: CENTER, label: "Envelope", hint: "Ed25519", tone: "primary" },
  { id: "wb", x: 930, y: CENTER, label: "Workbench", hint: "analyst" },
];

const W = 112;
const H = 52;

const fast = "M 70 160 H 490 L 640 70 L 790 160 H 930";
const slow = "M 70 160 H 490 L 640 250 L 790 160 H 930";

const toneStroke: Record<NonNullable<Node["tone"]>, string> = {
  primary: "stroke-primary",
  good: "stroke-signal-good",
  live: "stroke-signal-live",
};

export function PipelineDiagram() {
  const reduceMotion = useReducedMotion();

  return (
    <div className="w-full">
      <svg
        viewBox="0 0 1000 320"
        className="hidden w-full md:block"
        role="img"
        aria-label="An alert flows from CloudTrail through the collector, Kafka and detection, forks to the classifier or the LLM, and both paths end in a signed envelope shown in the workbench."
      >
        <path d={fast} fill="none" className="stroke-border" strokeWidth="1.5" />
        <path d={slow} fill="none" className="stroke-border" strokeWidth="1.5" />
        <path
          d={fast}
          fill="none"
          className="flow-dash stroke-signal-good/70"
          strokeWidth="1.5"
        />
        <path
          d={slow}
          fill="none"
          className="flow-dash stroke-signal-live/60"
          strokeWidth="1.5"
        />

        {!reduceMotion ? (
          <>
            {[0, 1.2, 2.4].map((begin) => (
              <circle key={`f${begin}`} r="5" className="fill-signal-good">
                <animateMotion dur="3.6s" begin={`${begin}s`} repeatCount="indefinite" path={fast} />
              </circle>
            ))}
            <circle r="5" className="fill-signal-live">
              <animateMotion dur="6s" begin="0.6s" repeatCount="indefinite" path={slow} />
            </circle>
          </>
        ) : null}

        {nodes.map((n) => (
          <g key={n.id} transform={`translate(${n.x - W / 2} ${n.y - H / 2})`}>
            <rect
              width={W}
              height={H}
              rx="10"
              className={`fill-card ${n.tone ? toneStroke[n.tone] : "stroke-border"}`}
              strokeWidth={n.tone ? 1.5 : 1}
            />
            <text
              x={W / 2}
              y={22}
              textAnchor="middle"
              className="fill-foreground text-[13px] font-semibold"
            >
              {n.label}
            </text>
            <text
              x={W / 2}
              y={39}
              textAnchor="middle"
              className="fill-muted-foreground font-mono text-[10px]"
            >
              {n.hint}
            </text>
          </g>
        ))}

        <text x="640" y="22" textAnchor="middle" className="fill-signal-good font-mono text-[10px] uppercase tracking-widest">
          known pattern
        </text>
        <text x="640" y="306" textAnchor="middle" className="fill-signal-live font-mono text-[10px] uppercase tracking-widest">
          structurally novel
        </text>
      </svg>

      <ol className="space-y-2 md:hidden">
        {nodes.map((n) => (
          <li
            key={n.id}
            className="flex items-center justify-between rounded-lg border border-border/70 bg-card/40 px-3 py-2"
          >
            <span className="text-sm font-semibold">{n.label}</span>
            <span className="font-mono text-[10px] text-muted-foreground">{n.hint}</span>
          </li>
        ))}
      </ol>
    </div>
  );
}
